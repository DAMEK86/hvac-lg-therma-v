use crate::config::{ThermaConfig, DEFAULT_BAUD_RATE};
use crate::modbus::*;
use crate::registers::{coil, discrete, holding, input, ModbusRegister};
use log::info;
use std::result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::time::timeout;
use tokio_modbus::client::{rtu, Reader};
use tokio_modbus::prelude::*;
use tokio_modbus::Slave;
use tokio_serial::SerialStream;

pub mod config;
mod modbus;
#[allow(dead_code)]
#[cfg(feature = "mqtt")]
pub mod mqtt;
pub mod registers;

pub type Result<T> = result::Result<T, String>;

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
    pub fn default(cfg: ThermaConfig) -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            sender,
            cfg,
            discrete_registers: vec![
                discrete::WaterFlowStatus::structure(),
                discrete::CompressorStatus::structure(),
                discrete::CoolingStatus::structure(),
                discrete::WaterPumpStatus::structure(),
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
                input::RoomAirTemperatureCircuit1::structure(),
                input::RoomAirTemperatureCircuit2::structure(),
                input::WaterInletTemperature::structure(),
                input::WaterOutletTemperature::structure(),
                input::OutdoorAirTemperature::structure(),
            ],
        }
    }

    pub async fn new(cfg: ThermaConfig, shutdown_listener: Arc<AtomicBool>) -> Result<Arc<Self>> {
        let therma_instance = Arc::new(Self::default(cfg.clone()));
        let instance = therma_instance.clone();
        let tx = instance.sender.clone();
        let req_timeout = Duration::from_millis(cfg.timeout_ms);
        tokio::spawn(async move {
            /*            let slave = Slave(cfg.slave_id);
                        let builder = tokio_serial::new(cfg.tty_path, DEFAULT_BAUD_RATE)
                            .timeout(Duration::from_millis(cfg.timeout_ms));
                        let port = SerialStream::open(&builder).unwrap();
                        let mut ctx = rtu::attach_slave(port, slave);
                        ThermaV::initialize_bus(req_timeout, &mut ctx).await;

                        let sleep_booleans_ms = Duration::from_millis(50);
                        let sleep_ms = Duration::from_millis(500);
            */
            while !shutdown_listener.load(Ordering::Relaxed) {
                /*for (topic, reg) in instance.coils.clone() {
                    match ThermaV::get_coil(req_timeout, &mut ctx, reg).await {
                        Ok(value) => {
                            match tx.send((Register::Coil(CoilRegister(reg, value)), topic)) {
                                Ok(_) => info!(target: "modbus:coil", "reg {}={}", reg, value),
                                Err(err) => log::error!(target: "modbus:coil", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:coil", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_booleans_ms).await;
                }

                for (topic, reg) in instance.discrete_registers.clone() {
                    match ThermaV::get_discrete(req_timeout, &mut ctx, reg).await {
                        Ok(value) => {
                            match tx.send((Register::Discrete(DiscreteRegister(reg, value)), topic)) {
                                Ok(_) => info!(target: "modbus:discrete", "reg {}={}", reg, value),
                                Err(err) => log::error!(target: "modbus:discrete", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:discrete", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_booleans_ms).await;
                }

                for (topic, reg) in instance.input_registers.clone() {
                    match ThermaV::get_input(req_timeout, &mut ctx, reg).await {
                        Ok(value) => {
                            match tx.send((Register::Input(InputRegister(reg, value.clone())), topic)) {
                                Ok(_) => {
                                    if reg == input::RoomAirTemperatureCircuit1::reg() {
                                        info!(target: "modbus:input", "RoomAirTemperatureCircuit1 {}={:?}", reg, input::RoomAirTemperatureCircuit1::from(value.clone()));
                                    }
                                    if reg == input::RoomAirTemperatureCircuit2::reg() {
                                        info!(target: "modbus:input", "RoomAirTemperatureCircuit2 {}={:?}", reg, input::RoomAirTemperatureCircuit2::from(value.clone()));
                                    }
                                    if reg == input::WaterInletTemperature::reg() {
                                        info!(target: "modbus:input", "WaterInletTemperature {}={:?}", reg, input::WaterInletTemperature::from(value.clone()));
                                    }
                                    if reg == input::WaterOutletTemperature::reg() {
                                        info!(target: "modbus:input", "WaterOutletTemperature {}={:?}", reg, input::WaterOutletTemperature::from(value.clone()));
                                    }
                                    if reg == input::OutdoorAirTemperature::reg() {
                                        info!(target: "modbus:input", "OutdoorAirTemperature {}={:?}", reg, input::OutdoorAirTemperature::from(value.clone()));
                                    }
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
                    match ThermaV::get_holding(req_timeout, &mut ctx, reg).await {
                        Ok(value) => {
                            match tx.send((Register::Holding(HoldingRegister(reg, value.clone())), topic)) {
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
                                },
                                Err(err) => log::error!(target: "modbus:holding", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:holding", "{}", err)
                        }
                    }
                    tokio::time::sleep(sleep_ms).await;
                }
                tx.send((Register::Discrete(DiscreteRegister(9999, true)), String::from("dhw_availability"))).expect("");
                */
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
        });

        Ok(therma_instance)
    }

    async fn water_flow_status(&mut self) -> Result<bool> {
        //ThermaV::get_discrete(self.)
        !todo!()
    }

    pub async fn initialize_bus(req_timeout: Duration, ctx: &mut client::Context) {
        info!("Starting Modbus initialization");
        for _ in 0..3 {
            let _ = timeout(req_timeout, ctx.read_coils(coil::EmergencyStop::reg(), 1)).await;
        }
    }

    pub async fn set_coil(
        req_timeout: Duration,
        ctx: &mut client::Context,
        reg: u16,
        value: bool,
    ) -> Result<()> {
        if timeout(req_timeout, ctx.write_single_coil(reg, value))
            .await
            .is_ok()
        {
            return Ok(());
        }
        Err(format!("set failed 0x{:02x}", reg))
    }

    pub async fn get_coil(
        req_timeout: Duration,
        ctx: &mut client::Context,
        reg: u16,
    ) -> Result<bool> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_coils(reg, 1)).await {
            return Ok(result[0]);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_discrete(
        req_timeout: Duration,
        ctx: &mut client::Context,
        reg: u16,
    ) -> Result<bool> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_discrete_inputs(reg, 1)).await {
            return Ok(result[0]);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_holding(
        req_timeout: Duration,
        ctx: &mut client::Context,
        reg: u16,
    ) -> Result<Vec<u16>> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_holding_registers(reg, 1)).await {
            return Ok(result);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_input(
        req_timeout: Duration,
        ctx: &mut client::Context,
        reg: u16,
    ) -> Result<Vec<u16>> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_input_registers(reg, 1)).await {
            return Ok(result);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }
}

impl SignalListener for ThermaV {
    fn register_receiver(&self) -> Receiver<(Register, String)> {
        self.sender.subscribe()
    }
}
