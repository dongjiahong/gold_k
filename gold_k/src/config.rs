use std::{env, str::FromStr, sync::Arc};

use tokio::{fs, sync::OnceCell};
use tracing::debug;
use validator::Validate;
#[derive(Debug, Clone, Validate, serde::Deserialize)]
pub struct Config {
    #[validate(length(min = 1))]
    pub database_url: String,
}

impl FromStr for Config {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str(s)
    }
}

pub static GLOBAL_CONFIG: OnceCell<Arc<Config>> = OnceCell::const_new();

pub async fn get_global_config() -> &'static Arc<Config> {
    let config_url = env::var("GOLD_K_CONFIG").expect("GOLD_K_CONFIG is not set");
    debug!("Loading global config from {}", config_url);
    GLOBAL_CONFIG
        .get_or_init(|| async {
            Arc::new(
                fs::read_to_string(&config_url)
                    .await
                    .expect("Faild to read config file")
                    .parse::<Config>()
                    .expect("Faild to parse config"),
            )
        })
        .await
}
