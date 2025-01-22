use crate::{mqtt, rwlock_read_guard, rwlock_write_guard, SignalListener};
use config::Map;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc};

#[derive(Serialize, Clone)]
pub struct Discovery {
    #[serde(rename = "dev")]
    pub device: Device,
    #[serde(rename = "o")]
    pub origin: Origin,
    #[serde(rename = "cmps")]
    pub components: Map<String, Component>,
}

impl From<&Discovery> for Vec<u8> {
    fn from(value: &Discovery) -> Self {
        let json = serde_json::to_string(value).unwrap();
        json.into_bytes()
    }
}

impl From<Discovery> for Vec<u8> {
    fn from(value: Discovery) -> Self {
        let json = serde_json::to_string(&value).unwrap();
        json.into_bytes()
    }
}

#[derive(Clone, Serialize)]
pub struct Origin {
    pub name: String,
    #[serde(rename = "sw", skip_serializing_if = "Option::is_none")]
    pub sw_version: Option<String>,
    #[serde(rename = "url", skip_serializing_if = "Option::is_none")]
    pub support_url: Option<String>,
}

#[derive(Serialize, Default, Clone)]
pub struct Component {
    pub name: String,
    pub object_id: String,
    pub unique_id: String,
    pub state_topic: String,
    #[serde(rename = "p")]
    pub platform: String,
    #[serde(rename = "avty_t")]
    pub availability_topic: String,
    #[serde(rename = "dev_cla", skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    #[serde(rename = "pl_avail")]
    pub payload_available: String,
    #[serde(rename = "pl_not_avail")]
    pub payload_not_available: String,
    #[serde(rename = "unit_of_meas", skip_serializing_if = "Option::is_none")]
    pub unit_of_measurement: Option<String>,
    pub icon: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<Vec<String>>,
    #[serde(rename = "mode_stat_t", skip_serializing_if = "Option::is_none")]
    pub mode_state_topic: Option<String>,
    #[serde(rename = "mode_cmd_t", skip_serializing_if = "Option::is_none")]
    pub mode_command_topic: Option<String>,
    #[serde(rename = "temp_stat_t", skip_serializing_if = "Option::is_none")]
    pub temperature_state_topic: Option<String>,
    #[serde(rename = "temp_cmd_t", skip_serializing_if = "Option::is_none")]
    pub temperature_command_topic: Option<String>,
    #[serde(rename = "curr_temp_t", skip_serializing_if = "Option::is_none")]
    pub current_temperature_topic: Option<String>,
}

impl Component {
    pub fn new(name: &str, device_name: &str, id: &str, icon: &str) -> Self {
        Self {
            name: String::from(name),
            object_id: format!("{}.{}", device_name, id),
            unique_id: format!("{}.{}", device_name, id),
            state_topic: format!("{}/{}.{}", device_name, device_name.to_lowercase(), id),
            availability_topic: format!("{}/$state", device_name),
            payload_available: String::from("ready"),
            payload_not_available: String::from("lost"),
            icon: icon.to_string(),
            ..Default::default()
        }
    }

    pub fn temperature_sensor(mut self) -> Self {
        self.platform = "sensor".to_string();
        self.device_class = Some("temperature".to_string());
        self.unit_of_measurement = Some("°C".to_string());
        self
    }

    pub fn binary_sensor(mut self) -> Self {
        self.platform = "binary_sensor".to_string();
        self.device_class = Some("opening".to_string());
        self
    }

    pub fn water_heater(mut self, modes: Vec<&str>) -> Self {
        self.platform = "water_heater".to_string();
        self.mode_state_topic = Some(format!("{}/mode", self.state_topic.clone()));
        self.mode_command_topic = Some(format!("{}/mode/set", self.state_topic.clone()));
        self.temperature_state_topic = Some(format!("{}/temperature", self.state_topic.clone()));
        self.temperature_command_topic =
            Some(format!("{}/temperature/set", self.state_topic.clone()));
        self.current_temperature_topic =
            Some(format!("{}/current_temperature", self.state_topic.clone()));
        self.modes = Some(modes.iter().map(|s| s.to_string()).collect());
        self
    }
}

#[derive(Clone, Serialize)]
pub struct Device {
    #[serde(rename = "ids")]
    pub identifiers: Vec<String>,
    pub name: String,
    #[serde(rename = "mf", skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(rename = "mdl", skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(rename = "mdl_id", skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(rename = "sw", skip_serializing_if = "Option::is_none")]
    pub sw_version: Option<String>,
    #[serde(rename = "sn", skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(rename = "hw", skip_serializing_if = "Option::is_none")]
    pub hw_version: Option<String>,
    #[serde(rename = "sa", skip_serializing_if = "Option::is_none")]
    pub suggested_area: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub model: String,
}

impl From<DeviceConfig> for Device {
    fn from(cfg: DeviceConfig) -> Self {
        Self {
            name: cfg.name,
            identifiers: vec![cfg.id],
            manufacturer: Some(cfg.manufacturer),
            model: Some(cfg.model),
            model_id: None,
            sw_version: None,
            serial_number: None,
            hw_version: None,
            suggested_area: None,
        }
    }
}

fn create_discovery_message() -> Discovery {
    let device_config: DeviceConfig = DeviceConfig {
        id: "lg_therma_v".to_string(),
        name: "ThermaV R32".to_string(),
        manufacturer: "LG".to_string(),
        model: "ThermaV R32".to_string(),
    };

    let mut map = Map::<String, Component>::new();
    map.insert(
        map.len().to_string(),
        Component::new(
            "Inlet Temperature",
            "ThermaV",
            "inlet_temperature",
            "mdi:water-thermometer",
        )
        .temperature_sensor(),
    );

    map.insert(
        map.len().to_string(),
        Component::new(
            "Water Flow Status",
            "ThermaV",
            "water_flow_status",
            "mdi:water-pump",
        )
        .binary_sensor(),
    );
    map.insert(
        map.len().to_string(),
        Component::new("DHW", "ThermaV", "dhw", "mdi:water-boiler").water_heater(vec![
            "off",
            "eco",
            "heat_pump",
            "performance",
        ]),
    );

    Discovery {
        device: device_config.into(),
        origin: Origin {
            name: "LG ThermaV".to_string(),
            sw_version: None,
            support_url: None,
        },
        components: map,
    }
}

#[derive(Serialize, Clone)]
struct BinarySensor(bool);

impl From<BinarySensor> for Vec<u8> {
    fn from(value: BinarySensor) -> Self {
        let mut state = "OFF";
        if value.0 {
            state = "ON";
        }
        state.as_bytes().to_vec()
    }
}

pub fn start_hass_mqtt_bridge_task<T>(
    mqtt_client: mqtt::Client,
    modbus_frame_listener: &T,
    signal: Arc<AtomicBool>,
) where
    T: SignalListener,
{
    let mqtt_client = Arc::new(tokio::sync::RwLock::new(mqtt_client));
    let mut modbus_rx = modbus_frame_listener.register_receiver();
    let mut hass_client = Hass::new(mqtt_client.clone(), String::from("ThermaV"));
    tokio::spawn(async move {
        let discovery_message = create_discovery_message();
        {
            let client = rwlock_read_guard(&mqtt_client).await;
            if let Err(error) = client
                .publish_with_base_topic(
                    format!("device/{}/config", String::from("lg_therma_v")),
                    discovery_message.clone(),
                )
                .await
            {
                log::error!(target: "mqtt-client", "failed to publish discovery msg: {error}");
            }
        }

        hass_client.subscribe(discovery_message).await;

        hass_client.publish_state(true).await;
        let mut state = BinarySensor(false);
        loop {
            if signal.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            if let Some(error) = hass_client
                .send_sensor_data("thermav.water_flow_status", state.clone())
                .await
            {
                log::error!(target: "mqtt-client", "failed to send data: {error}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            state.0 = !state.0;
        }

        /*
        loop {
            if signal.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let (reg, topic) = match modbus_rx.try_recv() {
                Ok((reg, topic)) => (reg, topic),
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    continue;
                }
            };
            match reg {
                Register::Coil(reg) => {
                    if let Some(error) = mqtt_client
                        .publish(format!("coils/{:03x}", reg.0), vec![reg.1 as u8])
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Discrete(reg) => {
                    if let Some(error) = mqtt_client
                        .publish(format!("discrete/{:03x}", reg.0), vec![reg.1 as u8])
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Holding(reg) => {
                    if let Some(error) = mqtt_client
                        .publish(
                            format!("holding/{:03x}", reg.0),
                            reg.1
                                .iter()
                                .flat_map(|&num| num.to_le_bytes())
                                .collect::<Vec<_>>(),
                        )
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Input(reg) => {
                    if let Some(error) = mqtt_client
                        .publish(
                            format!("input/{:03x}", reg.0),
                            reg.1
                                .iter()
                                .flat_map(|&num| num.to_le_bytes())
                                .collect::<Vec<_>>(),
                        )
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
            }

            if topic == "dhw_availability" {
                mqtt_client
                    .publish("ThermaV/dhw/availability", String::from("online"))
                    .await;
            }
        }
        */
    });
}

struct Hass {
    mqtt_client: Arc<tokio::sync::RwLock<mqtt::Client>>,
    instance_name: String,
}

impl Hass {
    pub fn new(mqtt_client: Arc<tokio::sync::RwLock<mqtt::Client>>, instance_name: String) -> Self {
        Self {
            mqtt_client,
            instance_name,
        }
    }

    pub async fn send_sensor_data<T>(&self, sensor_id: &str, state: T) -> Option<String>
    where
        T: Into<Vec<u8>>,
    {
        let locked_mqtt_client = rwlock_read_guard(&self.mqtt_client).await;
        locked_mqtt_client
            .publish(format!("{}/{}", self.instance_name, sensor_id), state)
            .await
    }

    pub async fn subscribe(&mut self, device_discovery: Discovery) {
        let mut client = rwlock_write_guard(&self.mqtt_client).await;
        for component in device_discovery.components {
            if let Some(topic) = component.1.mode_command_topic {
                if let Err(err) = client
                    .subscribe(
                        topic,
                        Arc::new(|topic, payload| {
                            println!("receive 'mode_command_topic' {}: {}", topic, payload);
                        }),
                    )
                    .await
                {
                    log::error!(target: "mqtt-client", "failed to subscribe msg: {err}");
                }
            }

            if let Some(topic) = component.1.temperature_command_topic {
                if let Err(err) = client
                    .subscribe(
                        topic,
                        Arc::new(|topic, payload| {
                            println!("receive 'temperature_command_topic' {}: {}", topic, payload);
                        }),
                    )
                    .await
                {
                    log::error!(target: "mqtt-client", "failed to subscribe msg: {err}");
                }
            }
        }
    }

    pub async fn publish_state(&self, state: bool) -> Option<String> {
        let mut value = "lost";
        if state {
            value = "ready";
        }

        let client = rwlock_read_guard(&self.mqtt_client).await;
        client
            .publish(
                format!("{}/$state", self.instance_name),
                value.as_bytes().to_vec(),
            )
            .await
    }
}
