use serde::Deserialize;
use tokio_modbus::Address;
use tokio_modbus::client::{Context, Reader};

#[derive(Deserialize)]
pub struct Register {
    pub name: String,
    pub addr: u16,
    pub data_type: String,
    pub factor: Option<String>,
    pub unit: Option<String>,
}

pub async fn reg_to_float(ctx: &mut Context, register: Address, factor: f32) -> Result<f32, Box<dyn std::error::Error>> {
    Ok(f32::from_be_bytes(
        read_reg(ctx, register, 2)
            .await?
            .try_into().unwrap(),
    ) * factor)
}

pub async fn reg_to_i16(ctx: &mut Context, register: Address, factor: i16) -> Result<i16, Box<dyn std::error::Error>> {
    Ok(i16::from_be_bytes(
        read_reg(ctx, register, 1)
            .await?
            .try_into()
            .unwrap(),
    ) * factor)
}

pub async fn reg_to_u16(ctx: &mut Context, register: Address) -> Result<u16, Box<dyn std::error::Error>> {
    Ok(u16::from_be_bytes(
        read_reg(ctx, register, 1)
            .await?
            .try_into()
            .unwrap(),
    ))
}

async fn read_reg(ctx: &mut Context, register: Address, cnt: u16) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let rsp = ctx.read_input_registers(register, cnt).await??;
    Ok(rsp
        .iter()
        .flat_map(|&w| [(w >> 8) as u8, w as u8])
        .collect::<Vec<_>>())
}