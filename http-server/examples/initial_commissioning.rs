extern crate thermav_lib;
use crate::thermav_lib::registers::ModbusRegister;
use std::thread::sleep;
use std::time::Duration;
use thermav_lib::registers::{coil, discrete, holding};
use tokio::task;
use tokio_modbus::client::sync::Context;
use tokio_modbus::prelude::*;
use tokio_modbus::Address;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tty_path = "/dev/ttyUSB0";
    let slave = Slave(1);
    let builder = tokio_serial::new(tty_path, thermav_lib::config::DEFAULT_BAUD_RATE);

    task::spawn_blocking(move || {
        let mut ctx = sync::rtu::connect_slave_with_timeout(
            &builder,
            slave,
            Some(Duration::from_millis(thermav_lib::config::DEFAULT_TIMEOUT)),
        )
        .unwrap();
        for _ in 0..3 {
            println!("test");
            let _ = ctx.read_coils(0x9, 1);
        }
        loop {
            println!("Reading ...");
            if let Ok(Ok(data)) = read_coil(&mut ctx, coil::EnableDisableHeatingCooling::reg()) {
                let res = coil::EnableDisableHeatingCooling::from(data);
                println!("Heating/Cooling: {res}")
            }
            sleep(Duration::from_millis(100));
            if let Ok(Ok(data)) = read_discrete(&mut ctx, discrete::CompressorStatus::reg()) {
                let res = discrete::CompressorStatus::from(data);
                println!("CompressorStatus: {res}")
            }
            sleep(Duration::from_millis(100));
            if let Ok(Ok(data)) = read_discrete(&mut ctx, discrete::WaterPumpStatus::reg()) {
                let res = discrete::WaterPumpStatus::from(data);
                println!("WaterPumpStatus: {res}")
            }
            sleep(Duration::from_millis(100));
            if let Ok(Ok(data)) = read_holding(&mut ctx, holding::ControlMethod::reg()) {
                let res = holding::ControlMethod::from(data);
                println!("Control Method: {res:?}")
            }
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 2, "Wasser Temp IN°C");
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 3, "Wasser Temp OUT °C");
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 9, "Durchfluss l/min");
            sleep(Duration::from_millis(100));
            if let Ok(Ok(data)) = read_holding(&mut ctx, holding::OperationMode::reg()) {
                let res = holding::OperationMode::from(data);
                println!("Operation Mode: {res:?}")
            }
            sleep(Duration::from_millis(100));
            //let d = ctx.write_single_register(0x2, 550);
            //if let Ok(_) = d {println!("ok")}
            sleep(Duration::from_secs(1));
            // an aus
            //let d = ctx.write_single_coil(0x0, false);
            //if let Ok(_) = d {println!("ok")}
        }
    })
    .await?;

    Ok(())
}

fn read_input(ctx: &mut Context, addr: Address, name: &str) {
    let resp = ctx.read_input_registers(addr, 1);
    if let Ok(data) = resp {
        println!("{name}: {data:?}")
    }
}

fn read_holding(ctx: &mut Context, addr: Address) -> tokio_modbus::Result<Vec<u16>> {
    ctx.read_holding_registers(addr, 1)
}

fn read_coil(ctx: &mut Context, addr: Address) -> tokio_modbus::Result<Vec<bool>> {
    ctx.read_coils(addr, 1)
}

fn read_discrete(ctx: &mut Context, addr: Address) -> tokio_modbus::Result<Vec<bool>> {
    ctx.read_discrete_inputs(addr, 1)
}
