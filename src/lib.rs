use crate::registers::{coil, discrete, holding, ModbusRegister};
use std::result;
use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use log::info;
use tokio_modbus::client::{rtu, Reader};
use tokio_modbus::Slave;
use tokio_serial::SerialStream;
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
};
use tokio::time::timeout;
use crate::config::{ThermaConfig, DEFAULT_BAUD_RATE};
use tokio_modbus::prelude::*;

pub mod config;
pub mod registers;
#[allow(dead_code)]
#[cfg(feature = "mqtt")]
pub mod mqtt;

pub type Result<T> = result::Result<T, String>;

#[derive(Clone)]
#[allow(unused)]
pub struct ThermaV {
    sender: Sender<Register>
}

#[derive(Clone)]
pub enum Register {
    Coil(u16, bool),
    Discrete(u16, bool),
    Holding(u16, Vec<u16>),
}

pub trait SignalListener {
    fn register_receiver(&self) -> Receiver<Register>;
}

impl ThermaV {
    pub async fn new(
        cfg: ThermaConfig,
        shutdown_listener: Arc<AtomicBool>
    ) -> Result<Self> {
        let (sender, _) = broadcast::channel(100);
        let tx = sender.clone();
        let instance = Self { sender };
        let req_timeout = Duration::from_millis(cfg.timeout_ms);
        tokio::spawn(async move {
            let slave = Slave(cfg.slave_id);
            let builder = tokio_serial::new(cfg.tty_path, DEFAULT_BAUD_RATE)
                .timeout(Duration::from_millis(cfg.timeout_ms));
            let port = SerialStream::open(&builder).unwrap();
            let mut ctx = rtu::attach_slave(port, slave);
            ThermaV::initialize_bus(req_timeout, &mut ctx).await;
            let coils: Vec<u16> = vec![
                coil::EnableDisableHeatingCooling::reg(),
                coil::SilentModeSet::reg()
            ];
            let discretes: Vec<u16> = vec![
                discrete::WaterFlowStatus::reg(),
                discrete::CompressorStatus::reg(),
                discrete::CoolingStatus::reg(),
                discrete::WaterPumpStatus::reg(),
                discrete::DefrostingStatus::reg(),
                discrete::BackupHeaterStep1Status::reg(),
                discrete::BackupHeaterStep2Status::reg(),
                discrete::DHWBoostHeaterStatus::reg(),
                discrete::ErrorStatus::reg(),
            ];
            let holdings: Vec<u16> = vec![
                holding::OperationMode::reg(),
                holding::ControlMethod::reg(),
                holding::TargetTempHeatingCoolingCircuit1::reg(),
                holding::TargetTempHeatingCoolingCircuit2::reg(),
            ];

            while !shutdown_listener.load(Ordering::Relaxed) {
                for coil_reg in coils.clone() {
                    match ThermaV::get_coil(req_timeout, &mut ctx, coil_reg).await {
                        Ok(coil_value) => {
                            if let Err(err) = tx.send(Register::Coil(coil_reg, coil_value)) {
                                log::error!(target: "modbus:coil", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:coil", "{}", err)
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                for discrete_reg in discretes.clone() {
                    match ThermaV::get_discrete(req_timeout, &mut ctx, discrete_reg).await {
                        Ok(coil_value) => {
                            if let Err(err) = tx.send(Register::Discrete(discrete_reg, coil_value)) {
                                log::error!(target: "modbus:discrete", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:discrete", "{}", err)
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                for holding_reg in holdings.clone() {
                    match ThermaV::get_holding(req_timeout, &mut ctx, holding_reg).await {
                        Ok(coil_value) => {
                            if let Err(err) = tx.send(Register::Holding(holding_reg, coil_value)) {
                                log::error!(target: "modbus:holding", "forwarding failed: {}", err)
                            }
                        }
                        Err(err) => {
                            log::error!(target: "modbus:holding", "{}", err)
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        });

        Ok(instance)
    }

    async fn initialize_bus(req_timeout: Duration, ctx : &mut client::Context) {
        info!("Starting Modbus initialization");
        for _ in 0..3 {
            let _ = timeout(req_timeout, ctx.read_coils(coil::EmergencyStop::reg(),1)).await;
        }
    }

    pub async fn set_coil(req_timeout: Duration, ctx : &mut client::Context, reg: u16, value: bool) -> Result<()>
    {
        if timeout(req_timeout, ctx.write_single_coil(reg, value)).await.is_ok() {
            return Ok(())
        }
        Err(format!("set failed 0x{:02x}", reg))
    }

    pub async fn get_coil(req_timeout: Duration, ctx : &mut client::Context, reg: u16) -> Result<bool> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_coils(reg, 1)).await {
            return Ok(result[0]);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_discrete(req_timeout: Duration, ctx : &mut client::Context, reg: u16) -> Result<bool> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_discrete_inputs(reg, 1)).await {
            return Ok(result[0]);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }

    pub async fn get_holding(req_timeout: Duration, ctx : &mut client::Context, reg: u16) -> Result<Vec<u16>> {
        if let Ok(Ok(Ok(result))) = timeout(req_timeout, ctx.read_holding_registers(reg, 1)).await {
            return Ok(result);
        }
        Err(format!("read failed 0x{:02x}", reg))
    }
}

impl SignalListener for ThermaV {
    fn register_receiver(&self) -> Receiver<Register> {
        self.sender.subscribe()
    }
}
