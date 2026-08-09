#[derive(Clone, Debug)]
pub struct BackendConfig {
    riot_api_key: Option<String>,
}

impl BackendConfig {
    pub fn from_environment() -> Self {
        let riot_api_key = std::env::var("RIOT_API_KEY")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        Self { riot_api_key }
    }

    pub fn riot_api_key(&self) -> Option<&str> {
        self.riot_api_key.as_deref()
    }

    pub fn api_key_configured(&self) -> bool {
        self.riot_api_key.is_some()
    }
}
