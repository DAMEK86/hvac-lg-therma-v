use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

pub const DEFAULT_BAUD_RATE: u32 = 9600;
pub const DEFAULT_TIMEOUT: u64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MqttConfig {
    pub client_name: String,
    pub topic: String,
    pub host_name: String,
    pub host_port: u16,
    pub username: String,
    pub password: String,
    pub channel_size: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub listen_address: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThermaConfig {
    pub tty_path: String,
    pub slave_id: u8,
    pub timeout_ms: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[allow(unused)]
pub struct AppConfig {
    pub http: HttpConfig,
    pub mqtt: MqttConfig,
    pub therma: ThermaConfig,
}

pub fn read_config() -> AppConfig {
    let config_file: String = env::var("CONFIGFILE").unwrap_or_else(|_| "src/config.toml".into());
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
