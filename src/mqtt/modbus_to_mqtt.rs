use crate::{Register, SignalListener};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub fn start_publish_task<T>(
    mqtt_client: Arc<super::Client>,
    can_frame_listener: &T,
    signal: Arc<AtomicBool>,
) where
    T: SignalListener,
{
    let mut modbus_rx = can_frame_listener.register_receiver();
    tokio::spawn(async move {
        loop {
            if signal.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let reg = match modbus_rx.try_recv() {
                Ok(reg) => reg,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    continue;
                }
            };
            match reg {
                Register::Coil(addr, value) => {
                    if let Some(error) = mqtt_client.publish(format!("coils/{:03x}", addr), vec![value as u8]).await {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Discrete(addr, value) => {
                    if let Some(error) = mqtt_client.publish(format!("discrete/{:03x}", addr), vec![value as u8]).await {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Holding(addr, value) => {
                    if let Some(error) = mqtt_client.publish(format!("holding/{:03x}", addr), value
                        .iter()
                        .flat_map(|&num| num.to_le_bytes())
                        .collect::<Vec<_>>()).await {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Input(addr, value) => {
                    if let Some(error) = mqtt_client.publish(format!("input/{:03x}", addr), value
                        .iter()
                        .flat_map(|&num| num.to_le_bytes())
                        .collect::<Vec<_>>()).await {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
            }
        }
    });
}

