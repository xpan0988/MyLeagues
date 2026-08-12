#[derive(Clone, Debug)]
pub struct BackendConfig {
    riot_api_key: Option<String>,
}

impl BackendConfig {
    pub fn from_environment() -> Self {
        Self {
            riot_api_key: resolve_api_key(
                std::env::var("RIOT_API_KEY").ok(),
                development_secret_key(),
            ),
        }
    }

    pub fn riot_api_key(&self) -> Option<&str> {
        self.riot_api_key.as_deref()
    }

    pub fn api_key_configured(&self) -> bool {
        self.riot_api_key.is_some()
    }
}

fn resolve_api_key(
    environment: Option<String>,
    development_file: Option<String>,
) -> Option<String> {
    normalize(environment).or_else(|| normalize(development_file))
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// This plaintext fallback is intentionally development-only and never enters
/// application state, SQLite, IPC, or diagnostics. Production still requires
/// RIOT_API_KEY from its environment.
fn development_secret_key() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/dev-secrets.toml"))
            .ok()
            .and_then(|contents| parse_development_secret(&contents))
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

fn parse_development_secret(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let line = line
            .split_once('#')
            .map_or(line, |(before, _)| before)
            .trim();
        let value = line
            .strip_prefix("riot_api_key")?
            .trim_start()
            .strip_prefix('=')?
            .trim();
        value
            .strip_prefix('"')?
            .strip_suffix('"')
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_development_secret, resolve_api_key};

    #[test]
    fn environment_key_overrides_local_development_secret() {
        assert_eq!(
            resolve_api_key(Some(" env-key ".into()), Some("file-key".into())),
            Some("env-key".into())
        );
        assert_eq!(
            resolve_api_key(None, Some(" file-key ".into())),
            Some("file-key".into())
        );
        assert_eq!(resolve_api_key(Some(" ".into()), None), None);
    }

    #[test]
    fn parses_only_the_expected_quoted_toml_key() {
        assert_eq!(
            parse_development_secret("# comment\nriot_api_key = \"example\" # local only"),
            Some("example".into())
        );
        assert_eq!(
            parse_development_secret("riot_api_key = \"\""),
            Some("".into())
        );
        assert_eq!(parse_development_secret("api_key = \"example\""), None);
    }
}
