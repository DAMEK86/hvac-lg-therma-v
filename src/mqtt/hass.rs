use crate::{Register, SignalListener};
use config::Map;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Serialize)]
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

#[derive(Serialize, Default)]
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
}

impl Component {
    pub fn new(device_name: String, id: String) -> Self {
        Self {
            object_id: format!("{}.{}", device_name, id),
            unique_id: format!("{}.{}", device_name, id),
            availability_topic: format!("{}/$state", device_name),
            payload_available: String::from("ready"),
            payload_not_available: String::from("lost"),
            ..Default::default()
        }
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

// publisher
// device config
// device state (online/offline)
// sensor mapping
fn create_discovery_message() -> Discovery {
    let device_config: DeviceConfig = DeviceConfig {
        id: "lg_therma_v".to_string(),
        name: "ThermaV R32".to_string(),
        manufacturer: "LG".to_string(),
        model: "ThermaV R32".to_string(),
    };

    let mut map = Map::<String, Component>::new();
    let mut comp = Component::new("ThermaV".to_string(), "inlet_temperature".to_string());
    comp.name = "Inlet Temperature".to_string();
    comp.state_topic = "ThermaV/inlet_temperature".to_string();
    comp.platform = "sensor".to_string();
    comp.device_class = Some("temperature".to_string());
    comp.unit_of_measurement = Some("°C".to_string());
    comp.icon = "mdi:water-thermometer".to_string();
    map.insert("inlet_temperature1234".to_string(), comp);

    map.insert(
        "water_flow1234".to_string(),
        Component {
            name: "Water Flow Status".to_string(),
            object_id: "thermav.water_flow_status".to_string(),
            unique_id: "thermav.water_flow_status".to_string(),
            state_topic: "ThermaV/thermav.water_flow_status".to_string(),
            platform: "binary_sensor".to_string(),
            availability_topic: "ThermaV/$state".to_string(),
            device_class: Some("opening".to_string()),
            payload_available: String::from("ready"),
            payload_not_available: String::from("lost"),
            unit_of_measurement: None,
            icon: "mdi:water-pump".to_string(),
        },
    );
    map.insert(
        "dhw1234".to_string(),
        Component {
            name: "DHW".to_string(),
            object_id: "thermav.dhw".to_string(),
            unique_id: "thermav.dhw".to_string(),
            state_topic: "ThermaV/dhw/availability".to_string(),
            availability_topic: "ThermaV/$state".to_string(),
            payload_available: String::from("ready"),
            payload_not_available: String::from("lost"),
            device_class: None,
            unit_of_measurement: None,
            platform: "water_heater".to_string(),
            icon: "mdi:water-boiler".to_string(),
        },
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
    mqtt_client: Arc<super::Client>,
    modbus_frame_listener: &T,
    signal: Arc<AtomicBool>,
) where
    T: SignalListener,
{
    let mut modbus_rx = modbus_frame_listener.register_receiver();
    tokio::spawn(async move {
        if let Err(error) = mqtt_client
            .publish_with_base_topic(
                format!("device/{}/config", String::from("lg_therma_v")),
                create_discovery_message(),
            )
            .await
        {
            log::error!(target: "mqtt-client", "failed to publish discovery msg: {error}");
        }
        let hass_client = Hass::new(mqtt_client, String::from("ThermaV"));
        let mut state = BinarySensor(false);
        loop {
            if signal.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            if let Some(error) = hass_client
                .send_sensor_data("thermav.water_flow_status".to_string(), state.clone())
                .await
            {
                log::error!(target: "mqtt-client", "failed to send data: {error}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            state.0 = !state.0;
        }

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
    });
}

struct Hass {
    mqtt_client: Arc<super::Client>,
    instance_name: String,
}

impl Hass {
    pub fn new(mqtt_client: Arc<super::Client>, instance_name: String) -> Self {
        Self {
            mqtt_client: mqtt_client.clone(),
            instance_name,
        }
    }

    pub async fn send_sensor_data1<T>(&self, sensor_id: String, state: T) -> Result<(), String>
    where
        T: Into<Vec<u8>>,
    {
        self.mqtt_client
            .publish_with_base_topic(
                format!("sensor/{}/{}", self.instance_name, sensor_id),
                state,
            )
            .await
    }

    pub async fn send_sensor_data<T>(&self, sensor_id: String, state: T) -> Option<String>
    where
        T: Into<Vec<u8>>,
    {
        self.mqtt_client
            .publish(format!("{}/{}", self.instance_name, sensor_id), state)
            .await
    }
}
