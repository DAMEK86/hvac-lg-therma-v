use std::ops::Deref;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use thermav_lib::config;
use thermav_lib::hass::start_hass_mqtt_bridge_task;
#[cfg(feature = "mqtt")]
use thermav_lib::mqtt;

use crate::api::start_service;
use thermav_lib::ThermaV;

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

    let modbus = ThermaV::new(cfg.therma, interrupted.clone())
        .await
        .unwrap_or_else(|e| {
            log::error!(target: "main", "Unable to initialize modbus: {}", e);
            std::process::exit(1);
        });

    #[cfg(feature = "mqtt")]
    {
        #[allow(unused_variables)]
        let mqtt_client = mqtt::Client::new(&cfg.mqtt, interrupted.clone());

        #[cfg(not(feature = "hass"))]
        mqtt::modbus_to_mqtt::start_publish_task(mqtt_client, modbus.deref(), interrupted.clone());

        #[cfg(feature = "hass")]
        start_hass_mqtt_bridge_task(mqtt_client, modbus.deref(), interrupted.clone());
    }

    start_service(cfg.http).await;

    interrupted.store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(())
}
