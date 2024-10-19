use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use thermav_lib::config;
#[cfg(feature = "mqtt")]
use thermav_lib::mqtt;
use thermav_lib::ThermaV;
use crate::api::{start_service};

mod api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("RUST_LOG").is_err() {
        // set default to info if none is set already
        std::env::set_var("RUST_LOG", "info")
    }
    pretty_env_logger::init();
    let cfg = config::read_config();
    let interrupted = Arc::new(AtomicBool::new(false));
    

    let modbus = ThermaV::new(cfg.therma, interrupted.clone()).await
        .unwrap_or_else(|e| {
            log::error!(target: "main", "Unable to initialize modbus: {}", e);
            std::process::exit(1);
        });

    #[cfg(feature = "mqtt")]
    {
        #[allow(unused_variables)]
        let mqtt_client = mqtt::Client::new(&cfg.mqtt, interrupted.clone());
        
        mqtt::modbus_to_mqtt::start_publish_task(
            mqtt_client,
            &modbus,
            interrupted.clone(),
        );
    }
    
    start_service(cfg.http).await;

    interrupted.store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}
