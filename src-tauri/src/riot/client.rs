use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, StatusCode, Url};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

use crate::error::{AppError, AppResult, RiotApiError};

#[derive(Clone)]
pub struct RiotApiClient {
    http: Client,
    api_key: Arc<str>,
    limiter: Arc<RequestLimiter>,
}

struct RequestLimiter {
    state: Mutex<RateState>,
    short_window: RateWindow,
    long_window: RateWindow,
}

struct RateState {
    short: VecDeque<Instant>,
    long: VecDeque<Instant>,
}
#[derive(Clone, Copy)]
struct RateWindow {
    maximum: usize,
    duration: Duration,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RiotRequestTiming {
    pub network: Duration,
    pub rate_limit_wait: Duration,
    pub retry_backoff: Duration,
    pub deserialize: Duration,
}

#[derive(Debug)]
pub struct TimedRiotResponse<T> {
    pub data: T,
    pub timing: RiotRequestTiming,
}

impl RequestLimiter {
    fn production() -> Self {
        Self {
            state: Mutex::new(RateState {
                short: VecDeque::new(),
                long: VecDeque::new(),
            }),
            short_window: RateWindow {
                maximum: 20,
                duration: Duration::from_secs(1),
            },
            long_window: RateWindow {
                maximum: 100,
                duration: Duration::from_secs(120),
            },
        }
    }

    async fn acquire(&self) -> Duration {
        let started = Instant::now();
        loop {
            let wait = {
                let now = Instant::now();
                let mut state = self.state.lock().await;
                prune(&mut state.short, now, self.short_window.duration);
                prune(&mut state.long, now, self.long_window.duration);
                let short_wait = required_wait(&state.short, now, self.short_window);
                let long_wait = required_wait(&state.long, now, self.long_window);
                match (short_wait, long_wait) {
                    (None, None) => {
                        state.short.push_back(now);
                        state.long.push_back(now);
                        return Instant::now().duration_since(started);
                    }
                    _ => short_wait
                        .into_iter()
                        .chain(long_wait)
                        .max()
                        .unwrap_or_default(),
                }
            };
            sleep(wait.max(Duration::from_millis(1))).await;
        }
    }
}

fn prune(requests: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while requests
        .front()
        .is_some_and(|instant| now.duration_since(*instant) >= window)
    {
        requests.pop_front();
    }
}

fn required_wait(
    requests: &VecDeque<Instant>,
    now: Instant,
    window: RateWindow,
) -> Option<Duration> {
    (requests.len() >= window.maximum).then(|| {
        requests
            .front()
            .map(|oldest| (*oldest + window.duration).saturating_duration_since(now))
            .unwrap_or_default()
    })
}

impl RiotApiClient {
    pub fn new(api_key: impl Into<Arc<str>>) -> AppResult<Self> {
        let http = Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("MyLeague/0.1")
            .build()?;
        Ok(Self {
            http,
            api_key: api_key.into(),
            limiter: Arc::new(RequestLimiter::production()),
        })
    }

    pub async fn get<T>(
        &self,
        service: &'static str,
        host: &str,
        path_segments: &[&str],
        sanitized_path: &str,
        query: &[(&str, String)],
    ) -> AppResult<T>
    where
        T: DeserializeOwned,
    {
        Ok(self
            .get_timed(service, host, path_segments, sanitized_path, query)
            .await?
            .data)
    }

    pub async fn get_timed<T>(
        &self,
        service: &'static str,
        host: &str,
        path_segments: &[&str],
        sanitized_path: &str,
        query: &[(&str, String)],
    ) -> AppResult<TimedRiotResponse<T>>
    where
        T: DeserializeOwned,
    {
        let mut url = Url::parse(&format!("https://{host}"))
            .map_err(|error| AppError::Configuration(error.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| AppError::Configuration("invalid Riot API host".to_owned()))?
            .extend(path_segments);

        let mut timing = RiotRequestTiming::default();
        for attempt in 0..3_u32 {
            timing.rate_limit_wait += self.limiter.acquire().await;
            tracing::info!(target: "riot_api", service, host, path = sanitized_path, attempt = attempt + 1, "sending Riot API request");
            let network_started = Instant::now();
            let response = self.http.get(url.clone()).query(query)
                .header("X-Riot-Token", self.api_key.as_ref()).send().await.map_err(|error| {
                    tracing::warn!(target: "riot_api", service, host, path = sanitized_path, error = %error, "Riot API transport failed");
                    AppError::Network(error)
                })?;
            let status = response.status();
            tracing::info!(target: "riot_api", service, host, path = sanitized_path, status = status.as_u16(), "received Riot API response");

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(2);
                timing.network += network_started.elapsed();
                let backoff = Duration::from_secs(retry_after);
                sleep(backoff).await;
                timing.retry_backoff += backoff;
                continue;
            }
            if status.is_server_error() && attempt < 2 {
                timing.network += network_started.elapsed();
                let backoff = Duration::from_millis(500 * 2_u64.pow(attempt));
                sleep(backoff).await;
                timing.retry_backoff += backoff;
                continue;
            }
            let body = match response.bytes().await {
                Ok(body) => body,
                Err(error) if !status.is_success() => {
                    return Err(RiotApiError {
                        service,
                        host: host.to_owned(),
                        path: sanitized_path.to_owned(),
                        status: status.as_u16(),
                        body: format!("<response body unavailable: {error}>"),
                    }
                    .into());
                }
                Err(error) => return Err(AppError::Network(error)),
            };
            timing.network += network_started.elapsed();
            let deserialize_started = Instant::now();
            let data = decode_payload(service, host, sanitized_path, status, &body)?;
            timing.deserialize += deserialize_started.elapsed();
            return Ok(TimedRiotResponse { data, timing });
        }
        Err(RiotApiError {
            service,
            host: host.to_owned(),
            path: sanitized_path.to_owned(),
            status: 429,
            body: "rate limit retry budget exhausted".to_owned(),
        }
        .into())
    }
}

fn decode_payload<T>(
    service: &'static str,
    host: &str,
    path: &str,
    status: StatusCode,
    body: &[u8],
) -> AppResult<T>
where
    T: DeserializeOwned,
{
    if status.is_success() {
        return Ok(serde_json::from_slice(body)?);
    }
    let body = if body.is_empty() {
        "<empty>".to_owned()
    } else {
        let text = String::from_utf8_lossy(body);
        text.chars().take(4_096).collect()
    };
    Err(RiotApiError {
        service,
        host: host.to_owned(),
        path: path.to_owned(),
        status: status.as_u16(),
        body,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use super::{RateState, RateWindow, RequestLimiter, decode_payload};
    use crate::error::AppError;
    use reqwest::StatusCode;
    use serde::Deserialize;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::time::{Instant, sleep};

    struct LegacyLimiter {
        next: Mutex<Instant>,
    }
    impl LegacyLimiter {
        async fn acquire(&self) -> Duration {
            let started = Instant::now();
            let mut next = self.next.lock().await;
            let now = Instant::now();
            if *next > now {
                sleep(*next - now).await;
            }
            *next = Instant::now() + Duration::from_millis(1_250);
            started.elapsed()
        }
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Success {
        value: i64,
    }

    #[test]
    fn decodes_200_json_success() {
        let value: Success = decode_payload(
            "test",
            "oc1.api.riotgames.com",
            "/success",
            StatusCode::OK,
            br#"{"value":7}"#,
        )
        .unwrap();
        assert_eq!(value, Success { value: 7 });
    }

    #[test]
    fn preserves_403_json_error_as_structured_riot_error() {
        assert_riot_error(
            br#"{"status":{"message":"Forbidden","status_code":403}}"#,
            "{\"status\":{\"message\":\"Forbidden\",\"status_code\":403}}",
        );
    }

    #[test]
    fn preserves_403_non_json_error_as_structured_riot_error() {
        assert_riot_error(b"Forbidden by gateway", "Forbidden by gateway");
    }

    #[test]
    fn represents_empty_403_body_without_json_decode_error() {
        assert_riot_error(b"", "<empty>");
    }

    #[tokio::test]
    async fn global_rate_limiter_enforces_configured_window() {
        let limiter = RequestLimiter {
            state: Mutex::new(RateState {
                short: VecDeque::new(),
                long: VecDeque::new(),
            }),
            short_window: RateWindow {
                maximum: 2,
                duration: Duration::from_millis(40),
            },
            long_window: RateWindow {
                maximum: 100,
                duration: Duration::from_secs(1),
            },
        };
        assert!(limiter.acquire().await < Duration::from_millis(10));
        assert!(limiter.acquire().await < Duration::from_millis(10));
        let started = Instant::now();
        limiter.acquire().await;
        assert!(started.elapsed() >= Duration::from_millis(30));
    }

    #[tokio::test]
    async fn controlled_serial_vs_bounded_five_scheduler_benchmark() {
        const MATCHES: usize = 5;
        const NETWORK: Duration = Duration::from_millis(200);
        let legacy = LegacyLimiter {
            next: Mutex::new(Instant::now()),
        };
        let old_started = Instant::now();
        let mut old_wait = Duration::ZERO;
        for _ in 0..MATCHES {
            old_wait += legacy.acquire().await;
            sleep(NETWORK).await;
        }
        let old_elapsed = old_started.elapsed();

        let limiter = Arc::new(RequestLimiter::production());
        let new_started = Instant::now();
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..MATCHES {
            let limiter = Arc::clone(&limiter);
            tasks.spawn(async move {
                let wait = limiter.acquire().await;
                sleep(NETWORK).await;
                wait
            });
        }
        let mut new_wait = Duration::ZERO;
        while let Some(result) = tasks.join_next().await {
            new_wait += result.unwrap();
        }
        let new_elapsed = new_started.elapsed();
        eprintln!(
            "controlled_sync_benchmark matches={MATCHES} network_ms=200 old_elapsed_ms={} old_wait_ms={} old_throughput={:.2} new_elapsed_ms={} new_wait_ms={} new_throughput={:.2}",
            old_elapsed.as_millis(),
            old_wait.as_millis(),
            MATCHES as f64 / old_elapsed.as_secs_f64(),
            new_elapsed.as_millis(),
            new_wait.as_millis(),
            MATCHES as f64 / new_elapsed.as_secs_f64()
        );
        assert!(new_elapsed < old_elapsed / 2);
    }

    fn assert_riot_error(body: &[u8], expected: &str) {
        let result = decode_payload::<Success>(
            "league-v4",
            "oc1.api.riotgames.com",
            "/lol/league/v4/entries/by-puuid/{redacted-puuid}",
            StatusCode::FORBIDDEN,
            body,
        );
        match result {
            Err(AppError::RiotApi(error)) => {
                assert_eq!(error.status, 403);
                assert_eq!(error.service, "league-v4");
                assert_eq!(error.body, expected);
            }
            other => panic!("expected structured Riot API error, got {other:?}"),
        }
    }
}
