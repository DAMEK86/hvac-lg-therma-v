use crate::registers::{coil, holding, ModbusRegister};
use crate::{mqtt, rwlock_read_guard, rwlock_write_guard, Register, SignalListener, ThermaV};
use config::Map;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

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

    #[serde(rename = "ops", skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(rename = "cmd_t", skip_serializing_if = "Option::is_none")]
    pub command_topic: Option<String>,
}

impl Component {
    pub fn new(name: &str, device_name: &str, device_id: &str, id: &str, icon: &str) -> Self {
        Self {
            name: String::from(name),
            object_id: format!("{}.{}", device_name, id),
            unique_id: format!("{}.{}", device_name, id),
            state_topic: format!("{}/{}.{}", device_name, device_id, id),
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

    pub fn select(mut self, options: Vec<&str>) -> Self {
        self.platform = "select".to_string();
        self.options = Some(options.iter().map(|s| s.to_string()).collect());
        self.command_topic = Some(format!("{}/mode", self.state_topic.clone()));
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

pub trait DeviceProperties {
    fn base_topic(&self) -> String;
    fn id(&self) -> String;
    fn name(&self) -> String;
    fn manufacturer(&self) -> String;
    fn origin_name(&self) -> String;
    fn model(&self) -> String;
}

fn create_discovery_message(device_properties: &impl DeviceProperties) -> Discovery {
    let device_config: DeviceConfig = DeviceConfig {
        id: device_properties.id(),
        name: device_properties.name(),
        manufacturer: device_properties.manufacturer(),
        model: device_properties.model(),
    };

    let mut map = Map::<String, Component>::new();
    map.insert(
        map.len().to_string(),
        Component::new(
            "Inlet Temperature",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "water_inlet_temperature",
            "mdi:water-thermometer",
        )
        .temperature_sensor(),
    );

    map.insert(
        map.len().to_string(),
        Component::new(
            "Outlet Temperature",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "water_outlet_temperature",
            "mdi:water-thermometer",
        )
        .temperature_sensor(),
    );

    map.insert(
        map.len().to_string(),
        Component::new(
            "Water Flow Status",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "water_flow_status",
            "mdi:waves-arrow-right",
        )
        .binary_sensor(),
    );
    map.insert(
        map.len().to_string(),
        Component::new(
            "Water Pump Status",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "water_pump_status",
            "mdi:heat-pump",
        )
        .binary_sensor(),
    );
    map.insert(
        map.len().to_string(),
        Component::new(
            "Compressor Status",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "compressor_status",
            "mdi:arrow-collapse-all",
        )
        .binary_sensor(),
    );
    map.insert(
        map.len().to_string(),
        Component::new(
            "DHW Heating Status",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "d_h_w_heating_status_d_h_w_thermal_on_off",
            "mdi:water-boiler",
        )
        .binary_sensor(),
    );
    map.insert(
        map.len().to_string(),
        Component::new(
            "DHW",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "dhw",
            "mdi:water-boiler",
        )
        .water_heater(vec!["off", "heat_pump"]),
    );

    map.insert(
        map.len().to_string(),
        Component::new(
            "Energy State Input",
            device_properties.base_topic().as_str(),
            device_config.id.as_str(),
            "energy_mode",
            "mdi:battery-charging-high",
        )
        .select(vec![
            "Not Use",
            "Forced Off",
            "Normal Operation",
            "On-recommendation",
            "On-command step 2",
            "On-recommendation Step 1",
            "Energy Saving mode",
            "Super Energy saving mode",
        ]),
    );

    Discovery {
        device: device_config.into(),
        origin: Origin {
            name: device_properties.origin_name(),
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

pub fn start_hass_mqtt_bridge_task(
    therma: ThermaV,
    mqtt_client: mqtt::Client,
    mut modbus_rx: Receiver<(Register, String)>,
    mut mqtt_rx: Receiver<(String, String)>,
    signal: Arc<AtomicBool>,
) {
    let mqtt_client = Arc::new(tokio::sync::RwLock::new(mqtt_client));
    let therma_clone = therma.clone();
    let mut hass_client = Hass::new(mqtt_client.clone(), String::from(&therma.base_topic()));
    let forwarder_signal = signal.clone();

    // mqtt -> modbus
    tokio::spawn(async move {
        /**        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                if let Err(err) = therma.set_coil(coil::EnableDisableHeatingCooling::reg(), true).await {
                    log::error!(target: "mqtt-client", "failed to enable pump: {err}");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                if let Err(err) = therma.set_coil(coil::SilentModeSet::reg(), true).await {
                    log::error!(target: "mqtt-client", "failed to enable silent mode: {err}");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                if let Err(err) = therma.set_register(holding::OperationMode::reg(), 4u16).await {
                    log::error!(target: "mqtt-client", "failed to set operation mode to Heating: {err}");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if let Err(err) = therma.set_register(holding::ControlMethod::reg(), 1u16).await {
                    log::error!(target: "mqtt-client", "failed to set control mode to room air: {err}");
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if let Err(err) = therma.set_register(holding::EnergyStateInput::reg(), 2u16).await {
                    log::error!(target: "mqtt-client", "failed to set EnergyStateInput to normal: {err}");
                }
        */
        loop {
            if forwarder_signal.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            let (topic, payload) = match mqtt_rx.try_recv() {
                Ok((topic, payload)) => (topic, payload),
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    continue;
                }
            };
            // TODO: rework separation of concern.
            let _ = match topic.as_str() {
                "ThermaV/thermav.dhw/temperature/set" => therma
                    .set_coil(
                        holding::TargetTempHeatingCoolingCircuit2::reg(),
                        payload.eq("true"),
                    )
                    .await
                    .map_err(|err| err.to_string()),
                &_ => Ok(()),
            };

            println!("{}: {}", topic, payload);
        }
    });
    tokio::spawn(async move {
        let discovery_message = create_discovery_message(&therma_clone);
        {
            let client = rwlock_read_guard(&mqtt_client).await;
            if let Err(error) = client
                .publish_with_base_topic(
                    format!("device/{}/config", &therma_clone.id()),
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

        if let Some(error) = hass_client
            .send_sensor_data("thermav.dhw/mode", "heat_pump")
            .await
        {
            log::error!(target: "mqtt-client", "failed to send data: {error}");
        }

        // modbus -> mqtt
        loop {
            if signal.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            #[cfg(not(feature = "io"))]
            {
                if let Some(error) = hass_client
                    .send_sensor_data("thermav.water_flow_status", state.clone())
                    .await
                {
                    log::error!(target: "mqtt-client", "failed to send data: {error}");
                }
                state.0 = !state.0;
            }

            let (reg, topic) = match modbus_rx.try_recv() {
                Ok((reg, topic)) => (reg, topic),
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                    continue;
                }
            };
            let normalized_topic = format!("{}.{}", therma_clone.id(), &topic);
            match reg {
                Register::Coil(reg) => {
                    if let Some(error) = hass_client
                        .send_sensor_data(&normalized_topic, BinarySensor(reg.1))
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Discrete(reg) => {
                    if let Some(error) = hass_client
                        .send_sensor_data(&normalized_topic, BinarySensor(reg.1))
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Holding(reg) => {
                    let value = reg.1[0] as f64 * 0.1;
                    log::info!(target: "mqtt-client", "{}={}",normalized_topic, value);
                    if let Some(error) = hass_client
                        .send_sensor_data(&normalized_topic, value.to_string())
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
                Register::Input(reg) => {
                    let value = reg.1[0] as f64 * 0.1;
                    log::info!(target: "mqtt-client", "{}={}",normalized_topic, value);
                    if let Some(error) = hass_client
                        .send_sensor_data(&normalized_topic, value.to_string())
                        .await
                    {
                        log::error!(target: "mqtt-client", "failed to publish mqtt msg: {error}");
                    }
                }
            }

            //tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
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
                if let Err(err) = client.subscribe(topic).await {
                    log::error!(target: "mqtt-client", "failed to subscribe msg: {err}");
                }
            }

            if let Some(topic) = component.1.temperature_command_topic {
                if let Err(err) = client.subscribe(topic).await {
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
