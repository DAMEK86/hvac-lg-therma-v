use serde::{Deserialize, Serialize};

pub mod config;
pub mod registers;

#[derive(Clone, Serialize, Deserialize)]
pub struct ThermaV;

impl ThermaV {}
