pub mod modbus_to_mqtt;

use crate::config::MqttConfig;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Client {
    base_topic: String,
    mqtt_client: AsyncClient,
}

impl Client {
    pub fn new(config: &MqttConfig, shutdown_listener: Arc<AtomicBool>) -> Arc<Client> {
        let mut mqtt_options =
            MqttOptions::new(&config.client_name, &config.host_name, config.host_port);
        mqtt_options.set_credentials(&config.username, &config.password);

        let (client, event_loop) = AsyncClient::new(mqtt_options, config.channel_size);
        Self::start_event_loop(event_loop, shutdown_listener);
        Arc::new(Client {
            base_topic: config.topic.clone(),
            mqtt_client: client,
        })
    }

    fn start_event_loop(mut event_loop: EventLoop, shutdown_listener: Arc<AtomicBool>) {
        tokio::spawn(async move {
            while !shutdown_listener.load(Ordering::Relaxed) {
                if let Ok(_event) = event_loop.poll().await {}
            }
        });
    }

    pub async fn publish<Topic, Payload>(&self, topic: Topic, payload: Payload) -> Option<String>
    where
        Topic: Into<String>,
        Payload: Into<Vec<u8>> + Clone,
    {
        match self
            .mqtt_client
            .publish(format!("{}/{}", &self.base_topic, topic.into()), QoS::AtLeastOnce, false, payload.clone())
            .await
        {
            Ok(_) => None,
            Err(e) => Some(format!("Error publishing topic: {:?}", e)),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.mqtt_client.try_disconnect();
    }
}
