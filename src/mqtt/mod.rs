pub mod modbus_to_mqtt;

use crate::config::MqttConfig;
use rumqttc::{AsyncClient, ClientError, Event, EventLoop, MqttOptions, QoS};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

type Callback = Arc<dyn Fn(String, String) + Send + Sync>;

pub struct Client {
    base_topic: String,
    mqtt_client: AsyncClient,
    callbacks: Arc<tokio::sync::RwLock<HashMap<String, Callback>>>,
}

impl Client {
    pub fn new(
        config: &MqttConfig,
        shutdown_listener: Arc<AtomicBool>,
    ) -> (Self, Receiver<(String, String)>) {
        let mut mqtt_options =
            MqttOptions::new(&config.client_name, &config.host_name, config.host_port);
        mqtt_options.set_credentials(&config.username, &config.password);

        let (client, event_loop) = AsyncClient::new(mqtt_options, config.channel_size);

        let instance = Client {
            base_topic: config.topic.clone(),
            mqtt_client: client,
            callbacks: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };
        let (sender, receiver) = mpsc::channel(100);
        Self::start_event_loop(&instance, sender, event_loop, shutdown_listener);
        (instance, receiver)
    }

    fn start_event_loop(
        &self,
        sender: Sender<(String, String)>,
        mut event_loop: EventLoop,
        shutdown_listener: Arc<AtomicBool>,
    ) {
        tokio::spawn(async move {
            while !shutdown_listener.load(Ordering::Relaxed) {
                if let Ok(event) = event_loop.poll().await {
                    match event {
                        Event::Incoming(rumqttc::Packet::Publish(publish)) => {
                            let topic = publish.topic.clone();
                            let payload = String::from_utf8_lossy(&publish.payload).to_string();
                            if let Err(err) = sender.send((topic, payload.to_string())).await {
                                eprintln!("Error sending message: {}", err);
                            }
                        }
                        Event::Outgoing(_) => {}
                        _ => {}
                    }
                }
            }
        });
    }

    pub async fn publish<Topic, Payload>(&self, topic: Topic, payload: Payload) -> Option<String>
    where
        Topic: Into<String>,
        Payload: Into<Vec<u8>>,
    {
        match self
            .mqtt_client
            .publish(
                format!("{}", topic.into()),
                QoS::AtLeastOnce,
                false,
                payload,
            )
            .await
        {
            Ok(_) => None,
            Err(e) => Some(format!("Error publishing topic: {:?}", e)),
        }
    }

    pub async fn publish_with_base_topic<Topic, Payload>(
        &self,
        topic: Topic,
        payload: Payload,
    ) -> Result<(), String>
    where
        Topic: Into<String>,
        Payload: Into<Vec<u8>>,
    {
        match self
            .mqtt_client
            .publish(
                format!("{}/{}", &self.base_topic, topic.into()),
                QoS::AtLeastOnce,
                false,
                payload,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error publishing topic: {:?}", e)),
        }
    }

    pub async fn subscribe<Topic>(&mut self, topic: Topic) -> Result<(), ClientError>
    where
        Topic: Into<String>,
    {
        self.mqtt_client.subscribe(topic, QoS::AtLeastOnce).await
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.mqtt_client.try_disconnect();
    }
}
