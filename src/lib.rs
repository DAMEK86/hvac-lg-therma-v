use crate::config::{ThermaConfig, DEFAULT_BAUD_RATE, DEFAULT_TIMEOUT};
use crate::hass::DeviceProperties;
use crate::modbus::*;
use crate::registers::{coil, discrete, holding, input, ModbusRegister};
use log::info;
use std::ops::Deref;
use std::result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::timeout;
use tokio_modbus::client::{rtu, Reader};
use tokio_modbus::prelude::*;
use tokio_modbus::Slave;
use tokio_serial::SerialStream;

pub mod config;
#[cfg(feature = "hass")]
pub mod hass;
mod modbus;
#[allow(dead_code)]
#[cfg(feature = "mqtt")]
pub mod mqtt;
pub mod registers;

pub type Result<T> = result::Result<T, String>;

pub async fn rwlock_read_guard<T>(rwlock: &RwLock<T>) -> tokio::sync::RwLockReadGuard<'_, T> {
    rwlock.read().await
}

pub async fn rwlock_write_guard<T>(rwlock: &RwLock<T>) -> tokio::sync::RwLockWriteGuard<'_, T> {
    rwlock.write().await
}

struct HassProperty {
    register: Register,
}

#[derive(Clone, Debug)]
#[allow(unused)]
pub struct ThermaV {
    // register and topic
    sender: Sender<(Register, String)>,
    discrete_registers: Vec<(String, u16)>,
    coils: Vec<(String, u16)>,
    holding_registers: Vec<(String, u16)>,
    input_registers: Vec<(String, u16)>,
    cfg: ThermaConfig,
    ctx: Option<Arc<Mutex<client::Context>>>,
    req_timeout: Duration,
}

#[derive(Clone, Debug)]
pub enum Register {
    Coil(CoilRegister),
    Discrete(DiscreteRegister),
    Holding(HoldingRegister),
    Input(InputRegister),
}

pub trait SignalListener {
    fn register_receiver(&self) -> Receiver<(Register, String)>;
}

impl ThermaV {
    pub fn default(
        cfg: ThermaConfig,
        ctx: Option<Arc<Mutex<client::Context>>>,
    ) -> (Self, Receiver<(Register, String)>) {
        let (sender, receiver) = mpsc::channel(100);
        (
            Self {
                sender,
                cfg: cfg.clone(),
                discrete_registers: vec![
                    discrete::WaterFlowStatus::structure(),  // check
                    discrete::CompressorStatus::structure(), // check
                    discrete::CoolingStatus::structure(),
                    discrete::WaterPumpStatus::structure(), // check
                    discrete::DefrostingStatus::structure(),
                    discrete::BackupHeaterStep1Status::structure(),
                    discrete::BackupHeaterStep2Status::structure(),
                    discrete::DHWBoostHeaterStatus::structure(),
                    discrete::ErrorStatus::structure(),
                ],
                coils: vec![
                    coil::EnableDisableHeatingCooling::structure(),
                    coil::SilentModeSet::structure(),
                ],
                holding_registers: vec![
                    holding::OperationMode::structure(),
                    holding::ControlMethod::structure(),
                    holding::EnergyStateInput::structure(),
                    holding::TargetTempHeatingCoolingCircuit1::structure(),
                    holding::TargetTempHeatingCoolingCircuit2::structure(),
                    holding::RoomAirTempCircuit1::structure(),
                    holding::RoomAirTempCircuit2::structure(),
                    holding::DHWTargetTemp::structure(),
                    holding::ShiftValueTargetInAutoModeCircuit1::structure(),
                    holding::ShiftValueTargetInAutoModeCircuit2::structure(),
                ],
                input_registers: vec![
                    input::ErrorCode::structure(),
                    input::WaterInletTemperature::structure(),
                    input::WaterOutletTemperature::structure(),
                    input::BackupHeaterOutletTemperature::structure(),
                    input::DHWTankWaterTemperature::structure(),
                    input::SolarCollectorTemperature::structure(),
                    input::RoomAirTemperatureCircuit1::structure(),
                    input::CurrentFlowRate::structure(),
                    input::FlowTemperatureCircuit2::structure(),
                    input::RoomAirTemperatureCircuit2::structure(),
                    input::OutdoorAirTemperature::structure(),
                ],
                ctx: ctx.clone(),
                req_timeout: Duration::from_millis(cfg.timeout_ms),
            },
            receiver,
        )
    }

    pub async fn new(
        cfg: ThermaConfig,
        shutdown_listener: Arc<AtomicBool>,
    ) -> Result<(Self, Receiver<(Register, String)>)> {
        let mut thread_safe_ctx: Option<Arc<Mutex<client::Context>>> = None;
        #[cfg(feature = "io")]
        {
            let slave = Slave(cfg.slave_id);
            let builder = tokio_serial::new(cfg.tty_path.clone(), DEFAULT_BAUD_RATE)
                .timeout(Duration::from_millis(cfg.timeout_ms));
            let port = SerialStream::open(&builder).unwrap();
            let mut ctx = rtu::attach_slave(port, slave);
            thread_safe_ctx = Some(Arc::new(Mutex::new(ctx)));
        }
        let (therma_instance, receiver) = Self::default(cfg, thread_safe_ctx);
        let instance = therma_instance.clone();
        let tx = instance.sender.clone();
        #[cfg(not(feature = "io"))]
        tokio::spawn(async move {
            let (topic, reg) = holding::TargetTempHeatingCoolingCircuit2::structure();
            while !shutdown_listener.load(Ordering::Relaxed) {
                let topic = remap_topic_from_modbus(topic.clone());
                if topic.eq("dhw/temperature") {
                    tx.send((Register::Holding(HoldingRegister(reg, vec![43])), topic))
                        .await;
                }
                let (topic, reg) = input::RoomAirTemperatureCircuit2::structure();
                let topic = remap_topic_from_modbus(topic.clone());
                if topic.eq("dhw/current_temperature") {
                    tx.send((Register::Holding(HoldingRegister(reg, vec![35])), topic))
                        .await;
                }
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
        });
        #[cfg(feature = "io")]
        tokio::spawn(async move {
            instance.initialize_bus().await;

            let sleep_booleans_ms = Duration::from_millis(500);
            let sleep_ms = Duration::from_millis(50);
            while !shutdown_listener.load(Ordering::Relaxed) {
                for (topic, reg) in instance.coils.clone() {
                    match instance.get_coil(reg).await {
                        Ok(value) => {
                            match tx
                                .send((Register::Coil(CoilRegister(reg, value)), topic))
                                .await
                            {
                                Ok(_) => info!(target: "modbus:coil", "reg {}={}", reg, value),
                                Err(err) => {
                                    log::error!(target: "modbus:coil", "forwarding failed: {}", err)
                                }
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:coil", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_booleans_ms).await;
                }

                for (topic, reg) in instance.discrete_registers.clone() {
                    match instance.get_discrete(reg).await {
                        Ok(value) => {
                            match tx
                                .send((Register::Discrete(DiscreteRegister(reg, value)), topic))
                                .await
                            {
                                Ok(_) => info!(target: "modbus:discrete", "reg {}={}", reg, value),
                                Err(err) => {
                                    log::error!(target: "modbus:discrete", "forwarding failed: {}", err)
                                }
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:discrete", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_booleans_ms).await;
                }

                for (topic, reg) in instance.input_registers.clone() {
                    match instance.get_input(reg).await {
                        Ok(value) => {
                            match tx
                                .send((
                                    Register::Input(InputRegister(reg, value.clone())),
                                    topic.clone(),
                                ))
                                .await
                            {
                                Ok(_) => {
                                    info!(target: "modbus:input", "{}/{}={:?}", reg, topic, value);
                                }
                                Err(err) => {
                                    log::error!(target: "modbus:input", "{}", err)
                                }
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:input", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_ms).await;
                }

                for (topic, reg) in instance.holding_registers.clone() {
                    match instance.get_holding(reg).await {
                        Ok(value) => {
                            let topic = remap_topic_from_modbus(topic);
                            match tx
                                .send((
                                    Register::Holding(HoldingRegister(reg, value.clone())),
                                    topic,
                                ))
                                .await
                            {
                                Ok(_) => {
                                    if reg == holding::OperationMode::reg() {
                                        info!(target: "modbus:holding", "OperationMode {}={:?}", reg, holding::OperationMode::from(value.clone()));
                                    }
                                    if reg == holding::ControlMethod::reg() {
                                        info!(target: "modbus:holding", "ControlMethod {}={:?}", reg, holding::ControlMethod::from(value.clone()));
                                    }
                                    if reg == holding::EnergyStateInput::reg() {
                                        info!(target: "modbus:holding", "EnergyStateInput {}={:?}", reg, holding::EnergyStateInput::from(value.clone()));
                                    }
                                    if reg == holding::TargetTempHeatingCoolingCircuit1::reg() {
                                        info!(target: "modbus:holding", "TargetTempHeatingCoolingCircuit1 {}={:?}", reg, holding::TargetTempHeatingCoolingCircuit1::from(value.clone()));
                                    }
                                    if reg == holding::TargetTempHeatingCoolingCircuit2::reg() {
                                        info!(target: "modbus:holding", "TargetTempHeatingCoolingCircuit2 {}={:?}", reg, holding::TargetTempHeatingCoolingCircuit2::from(value.clone()));
                                    }
                                    if reg == holding::RoomAirTempCircuit1::reg() {
                                        info!(target: "modbus:holding", "RoomAirTempCircuit1 {}={:?}", reg, holding::RoomAirTempCircuit1::from(value.clone()));
                                    }
                                    if reg == holding::RoomAirTempCircuit2::reg() {
                                        info!(target: "modbus:holding", "RoomAirTempCircuit2 {}={:?}", reg, holding::RoomAirTempCircuit2::from(value.clone()));
                                    }
                                    if reg == holding::DHWTargetTemp::reg() {
                                        info!(target: "modbus:holding", "DHWTargetTemp {}={:?}", reg, holding::DHWTargetTemp::from(value.clone()));
                                    }
                                    if reg == holding::ShiftValueTargetInAutoModeCircuit1::reg() {
                                        info!(target: "modbus:holding", "ShiftValueTargetInAutoModeCircuit1 {}={:?}", reg, holding::ShiftValueTargetInAutoModeCircuit1::from(value.clone()));
                                    }
                                    if reg == holding::ShiftValueTargetInAutoModeCircuit2::reg() {
                                        info!(target: "modbus:holding", "ShiftValueTargetInAutoModeCircuit2 {}={:?}", reg, holding::ShiftValueTargetInAutoModeCircuit1::from(value.clone()));
                                    }
                                }
                                Err(err) => {
                                    log::error!(target: "modbus:holding", "forwarding failed: {}", err)
                                }
                            }
                        }
                        Err(err) => {
                            #[cfg(not(feature = "io"))]
                            {
                                let topic = remap_topic_from_modbus(topic);
                                if topic.eq("dhw/temperature") {
                                    tx.send((
                                        Register::Holding(HoldingRegister(reg, vec![0xA1])),
                                        topic,
                                    ))
                                    .await;
                                }
                            }
                            log::error!(target: "modbus:holding", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_ms).await;
                }
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
        });

        Ok((therma_instance, receiver))
    }

    async fn initialize_bus(&self) {
        info!("Starting Modbus initialization");
        for _ in 0..3 {
            if let Some(ctx) = &self.ctx {
                let _ = timeout(
                    self.req_timeout,
                    ctx.lock().await.read_coils(coil::EmergencyStop::reg(), 1),
                )
                .await;
            }
        }
    }

    pub async fn set_coil(&self, reg: u16, value: bool) -> Result<()> {
        if let Some(ctx) = &self.ctx {
            if timeout(
                self.req_timeout,
                ctx.lock().await.write_single_coil(reg, value),
            )
            .await
            .is_ok()
            {
                return Ok(());
            }
        }
        Err(format!("set failed 0x{:02x}", reg))
    }

    pub async fn set_register(&self, reg: u16, value: u16) -> Result<()> {
        if let Some(ctx) = &self.ctx {
            if timeout(
                self.req_timeout,
                ctx.lock().await.write_single_register(reg, value),
            )
            .await
            .is_ok()
            {
                return Ok(());
            }
        }
        Err(format!("set failed 0x{:02x}", reg))
    }

    pub async fn get_coil(&self, reg: u16) -> Result<bool> {
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) =
                timeout(self.req_timeout, ctx.lock().await.read_coils(reg, 1)).await
            {
                return Ok(result[0]);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) =
                timeout(self.req_timeout, ctx.lock().await.read_coils(reg, 1)).await
            {
                return Ok(result[0]);
            }
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_discrete(&self, reg: u16) -> Result<bool> {
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_discrete_inputs(reg, 1),
            )
            .await
            {
                return Ok(result[0]);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_discrete_inputs(reg, 1),
            )
            .await
            {
                return Ok(result[0]);
            }
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_holding(&self, reg: u16) -> Result<Vec<u16>> {
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_holding_registers(reg, 1),
            )
            .await
            {
                return Ok(result);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_holding_registers(reg, 1),
            )
            .await
            {
                return Ok(result);
            }
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_input(&self, reg: u16) -> Result<Vec<u16>> {
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_input_registers(reg, 1),
            )
            .await
            {
                return Ok(result);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(ctx) = &self.ctx {
            if let Ok(Ok(Ok(result))) = timeout(
                self.req_timeout,
                ctx.lock().await.read_input_registers(reg, 1),
            )
            .await
            {
                return Ok(result);
            }
        }
        Err(format!("read failed 0x{:02x}", reg))
    }
}

fn remap_topic_from_modbus(topic: String) -> String {
    match topic.as_str() {
        "operation_mode" => String::from(""),
        "target_temp_heating_cooling_circuit2" => String::from("dhw/temperature"),
        "room_air_temperature_circuit2" => String::from("dhw/current_temperature"),
        &_ => topic,
    }
}

impl DeviceProperties for ThermaV {
    fn base_topic(&self) -> String {
        String::from("ThermaV")
    }

    fn id(&self) -> String {
        String::from("thermav")
    }

    fn name(&self) -> String {
        String::from("ThermaV R32")
    }

    fn manufacturer(&self) -> String {
        String::from("LG")
    }

    fn origin_name(&self) -> String {
        String::from("LG ThermaV")
    }

    fn model(&self) -> String {
        String::from("ThermaV R32")
    }
}
