use std::thread::sleep;
use register::{reg_to_float, reg_to_i16};
use std::time::Duration;
use tokio::task;
use tokio_modbus::client::sync::Context;
use tokio_modbus::prelude::*;
use tokio_modbus::Address;

mod register;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tty_path = "/dev/ttyUSB0";
    let slave = Slave(1);
    let builder = tokio_serial::new(tty_path, 9600);

    task::spawn_blocking(move || {
        let mut ctx = sync::rtu::connect_slave_with_timeout(&builder, slave, Some(Duration::from_millis(1000))).unwrap();
        for _ in 0..3 {
            println!("test");
            let _ = ctx.read_coils(0x9, 1);
        }
        loop {
            println!("Reading ...");
            read_coil(&mut ctx, 0x0, "Heating/Cooling on/off");
            sleep(Duration::from_millis(100));
            read_discrete(&mut ctx, 3, "Compressor on/off");
            sleep(Duration::from_millis(100));
            read_discrete(&mut ctx, 1, "Wasserpumpe on/off");
            sleep(Duration::from_millis(100));
            read_holding(&mut ctx, 1, "Betriebsmodus");
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 2, "Wasser Temp IN°C");
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 3, "Wasser Temp OUT °C");
            sleep(Duration::from_millis(100));
            read_input(&mut ctx, 9, "Durchfluss l/min");
            sleep(Duration::from_millis(100));
            read_holding(&mut ctx, 0x0, "Operation Mode");
            sleep(Duration::from_millis(100));
            //let d = ctx.write_single_register(0x2, 550);
            //if let Ok(_) = d {println!("ok")}
            sleep(Duration::from_secs(1));
            // an aus
            //let d = ctx.write_single_coil(0x0, false);
            //if let Ok(_) = d {println!("ok")}
        }
    }).await?;

    Ok(())
}

fn read_coil(ctx: &mut Context, addr: Address, name: &str) {
    let resp = ctx.read_coils(addr, 1);
    if let Ok(data) = resp { println!("{name}: {data:?}") }
}

fn read_input(ctx: &mut Context, addr: Address, name: &str) {
    let resp = ctx.read_input_registers(addr, 1);
    if let Ok(data) = resp { println!("{name}: {data:?}") }
}

fn read_discrete(ctx: &mut Context, addr: Address, name: &str) {
    let resp = ctx.read_discrete_inputs(addr, 1);
    if let Ok(data) = resp { println!("{name}: {data:?}") }
}

fn read_holding(ctx: &mut Context, addr: Address, name: &str) {
    let resp = ctx.read_holding_registers(addr, 1);
    if let Ok(data) = resp { println!("{name}: {data:?}") }
}

async fn read_from_meter(mut ctx: client::Context) -> Result<(), Box<dyn std::error::Error>> {

    println!("Reading a sensor value");
    let rsp = ctx.read_holding_registers(0x2E, 1).await?;
    println!("Address: {rsp:?}");

    let rsp = ctx.read_holding_registers(0x2D, 1).await?;
    println!("Baud-rate: {rsp:?}");

    let rsp = ctx.read_holding_registers(0x0, 1).await?;
    println!("SoftwareVersion: {rsp:?}");
    println!("Freq: {}Hz", reg_to_float(&mut ctx, 0x2044, 0.01).await?);

    println!("Uab: {}V", reg_to_float(&mut ctx, 0x2000, 0.1).await?);
    println!("Ubc: {}V", reg_to_float(&mut ctx, 0x2002, 0.1).await?);
    println!("Uca: {}V", reg_to_float(&mut ctx, 0x2004, 0.1).await?);
    println!("Ua: {}V", reg_to_float(&mut ctx, 0x2006, 0.1).await?);
    println!("Ub: {}V", reg_to_float(&mut ctx, 0x2008, 0.1).await?);
    println!("Uc: {}V", reg_to_float(&mut ctx, 0x200A, 0.1).await?);
    println!("Pt: {}W\n----", reg_to_float(&mut ctx, 0x2012, 0.1).await?);
    println!("Pa: {}W", reg_to_float(&mut ctx, 0x2014, 0.1).await?);
    println!("Pb: {}W", reg_to_float(&mut ctx, 0x2016, 0.1).await?);
    println!("Pc: {}W\n----", reg_to_float(&mut ctx, 0x2018, 0.1).await?);
    println!("IrAt: {}", reg_to_i16(&mut ctx, 0x6, 1).await?);
    println!("UrAt: {}", reg_to_i16(&mut ctx, 0x7, 1).await?);

    Ok(())
}
