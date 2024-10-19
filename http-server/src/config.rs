use std::env;
use config::{Config, Environment, File};
use serde::{Deserialize};

#[derive(Debug, Deserialize)]
pub struct HttpConfig {
    pub listen_address: String,
    pub listen_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub http: HttpConfig,
}

pub fn read_config() -> AppConfig {
    let config_file: String = env::var("CONFIGFILE").unwrap_or_else(|_| "src/config.json".into());
    let s = Config::builder()
        // Add default file passed by env _or_ default in IDE
        .add_source(File::with_name(&config_file))
        .add_source(Environment::with_prefix("app"))
        .build()
        .unwrap_or_else(|e| {
            log::error!(target: "config", "Error reading config file: {}", e);
            std::process::exit(1);
        });

    s.try_deserialize().unwrap_or_else(|e| {
        log::error!(target: "config", "Error deserializing config file: {}", e);
        std::process::exit(1);
    })
}

#[test]
fn test_parse_config() {
    read_config();
}