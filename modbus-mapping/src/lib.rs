extern crate proc_macro;

use proc_macro2::Ident;
use quote::__private::TokenStream;
use quote::quote;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

// Define the structure for parsing JSON
#[derive(Debug, Deserialize)]
struct EnumData {
    description: String,
    reg: u16,
    #[serde(rename = "enum")]
    enum_values: Option<HashMap<String, u16>>
}

#[derive(Debug, Deserialize)]
struct FloatData {
    description: String,
    reg: u16,
    #[serde(rename = "type")]
    #[allow(unused)]
    data_type: Option<String>,
    gain: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct SignedData {
    description: String,
    reg: u16,
    #[serde(rename = "type")]
    #[allow(unused)]
    data_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UnsignedShortData {
    description: String,
    reg: u16,
    #[serde(rename = "type")]
    #[allow(unused)]
    data_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Registers {
    holding: Vec<HoldingRegister>,
    coil: Vec<CoilRegister>,
    discrete: Vec<DiscreteRegister>,
    input: Vec<InputRegister>
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum HoldingRegister {
    #[serde(rename = "enum")]
    Enum(EnumData),
    #[serde(rename = "float")]
    Float(FloatData),
    #[serde(rename = "i8")]
    SignedChar(SignedData),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum InputRegister {
    #[serde(rename = "enum")]
    Enum(EnumData),
    #[serde(rename = "float")]
    Float(FloatData),
    #[serde(rename = "i8")]
    SignedChar(SignedData),
    #[serde(rename = "u16")]
    UnsignedShort(UnsignedShortData)
}

#[derive(Debug, Deserialize)]
struct BooleanValues {
    #[serde(rename = "false")]
    pub r#false: String,
    #[serde(rename = "true")]
    pub r#true: String,
}

#[derive(Debug, Deserialize)]
struct CoilRegister {
    pub description: String,
    pub reg: u16,
    pub values: BooleanValues,
}

#[derive(Debug, Deserialize)]
struct DiscreteRegister {
    pub description: String,
    pub reg: u16,
    pub values: BooleanValues,
}

fn sanitize_identifier(name: &str) -> String {
    let words: Vec<String> = name
        .split_whitespace()
        .map(|word| {
            let sanitized_word = word
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>();
            let mut chars = sanitized_word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();
    words.concat().replace('_', "")
}

pub fn generate_registers(modbus_register_data_file_path: &str) -> TokenStream {
    // Read the JSON file
    let json_data = fs::read_to_string(modbus_register_data_file_path).expect("Unable to read JSON file");
    let parsed: Registers = serde_json::from_str(&json_data).expect("Invalid JSON format");

    let mut holding_generated_enums: Vec<TokenStream> = Vec::new();
    let mut holding_generated_structs: Vec<TokenStream> = Vec::new();
    let mut coil_generated_structs: Vec<TokenStream> = Vec::new();
    let mut discrete_generated_structs: Vec<TokenStream> = Vec::new();
    let mut input_generated_enums: Vec<TokenStream> = Vec::new();
    let mut input_generated_structs: Vec<TokenStream> = Vec::new();

    for entry in parsed.holding {
        match entry {
            HoldingRegister::Enum(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );

                let reg_value = reg.reg;
                if let Some(reg) = reg.enum_values {
                    generate_enum(&mut holding_generated_enums, name, reg, reg_value);
                }
            },
            HoldingRegister::Float(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );

                let gain_value: f32 = reg.gain.unwrap_or(1f32);
                let reg_value = reg.reg;

                holding_generated_structs.push(quote! {
                    #[allow(unused)]
                    #[derive(Debug)]
                    pub struct #name(f32);

                    impl ModbusRegister<Vec<u16>> for #name {
                        fn reg() -> u16 { #reg_value }
                    }

                    impl From<Vec<u16>> for #name {
                        fn from(value: Vec<u16>) -> Self {
                            #name(value[0] as f32 * #gain_value)
                        }
                    }
                });

            }
            HoldingRegister::SignedChar(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );
                let reg_value = reg.reg;

                holding_generated_structs.push(quote! {
                    #[allow(unused)]
                    #[derive(Debug)]
                    pub struct #name(i16);

                    impl #name {
                        pub fn reg() -> u16 { #reg_value }
                    }

                    impl From<Vec<u16>> for #name {
                        fn from(value: Vec<u16>) -> Self {
                            #name(super::register_to_i16(value))
                        }
                    }
                });
            }
        }
    }
    
    for entry in parsed.input {
        match entry {
            InputRegister::Enum(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );

                let reg_value = reg.reg;
                if let Some(reg) = reg.enum_values {
                    generate_enum(&mut input_generated_enums, name, reg, reg_value);
                }
            }
            InputRegister::Float(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );

                let gain_value: f32 = reg.gain.unwrap_or(1f32);
                let reg_value = reg.reg;

                input_generated_structs.push(quote! {
                    #[allow(unused)]
                    #[derive(Debug)]
                    pub struct #name(f32);

                    impl ModbusRegister<Vec<u16>> for #name {
                        fn reg() -> u16 { #reg_value }
                    }

                    impl From<Vec<u16>> for #name {
                        fn from(value: Vec<u16>) -> Self {
                            #name(value[0] as f32 * #gain_value)
                        }
                    }
                });
            }
            InputRegister::SignedChar(reg) => {
                let name = syn::Ident::new(
                    &sanitize_identifier(&reg.description),
                    proc_macro2::Span::call_site(),
                );
                let reg_value = reg.reg;

                input_generated_structs.push(quote! {
                    #[allow(unused)]
                    #[derive(Debug)]
                    pub struct #name(i16);

                    impl #name {
                        pub fn reg() -> u16 { #reg_value }
                    }

                    impl From<Vec<u16>> for #name {
                        fn from(value: Vec<u16>) -> Self {
                            #name(super::register_to_i16(value))
                        }
                    }
                });
            }
            InputRegister::UnsignedShort(reg) => {}
        }
    }

    for entry in parsed.coil {
        let name = syn::Ident::new(
            &sanitize_identifier(&entry.description),
            proc_macro2::Span::call_site(),
        );

        let reg_value = entry.reg;
        let true_value = entry.values.r#true;
        let false_value = entry.values.r#false;

        coil_generated_structs.push(quote! {
            pub struct #name(bool);

            impl From<Vec<bool>> for #name {
                fn from(value: Vec<bool>) -> Self {
                    #name(value[0])
                }
            }

            impl ModbusRegister<Vec<bool>> for #name {
                fn reg() -> u16 { #reg_value }
            }

            impl #name {
                fn as_str(&self) -> &'static str {
                    if self.0 {
                        #true_value
                    } else {
                        #false_value
                    }
                }
            }

            impl fmt::Display for #name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.as_str())
                }
            }
        })
    }

    for entry in parsed.discrete {
        let name = syn::Ident::new(
            &sanitize_identifier(&entry.description),
            proc_macro2::Span::call_site(),
        );

        let reg_value = entry.reg;
        let true_value = entry.values.r#true;
        let false_value = entry.values.r#false;

        discrete_generated_structs.push(quote! {
            pub struct #name(bool);

            impl From<Vec<bool>> for #name {
                fn from(value: Vec<bool>) -> Self {
                    #name(value[0])
                }
            }

            impl ModbusRegister<Vec<bool>> for #name {
                fn reg() -> u16 { #reg_value }
            }

            impl #name {
                fn as_str(&self) -> &'static str {
                    if self.0 {
                        #true_value
                    } else {
                        #false_value
                    }
                }
            }

            impl fmt::Display for #name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, "{}", self.as_str())
                }
            }
        })
    }

    // Accumulate generated enums
    quote! {
        pub fn register_to_bytes(register_data: Vec<u16>) -> Vec<u8> {
            register_data.iter()
            .flat_map(|&w| [(w >> 8) as u8, w as u8])
            .collect::<Vec<_>>()
        }
        pub fn register_to_f32(data: Vec<u16>) -> f32 {
            f32::from_be_bytes(register_to_bytes(data).try_into().unwrap())
        }
        pub fn register_to_u16(data: Vec<u16>) -> u16 {
            u16::from_be_bytes(register_to_bytes(data).try_into().unwrap())
        }
        pub fn register_to_i16(data: Vec<u16>) -> i16 {
            i16::from_be_bytes(register_to_bytes(data).try_into().unwrap())
        }

        pub trait ModbusRegister<T> : From<T> {
            fn reg() -> u16;
        }

        pub mod coil {
            use std::fmt;
            use crate::registers::ModbusRegister;
            #(#coil_generated_structs)*
        }

        pub mod discrete {
            use std::fmt;
            use crate::registers::ModbusRegister;
            #(#discrete_generated_structs)*
        }
        
        pub mod input{
            use crate::registers::ModbusRegister;
            #(#input_generated_enums)*
            #(#input_generated_structs)*
        }

        pub mod holding{
            use crate::registers::ModbusRegister;
            #(#holding_generated_enums)*
            #(#holding_generated_structs)*
        }
    }
}

fn generate_enum(
    generated_enums: &mut Vec<TokenStream>,
    enum_name: Ident,
    reg: HashMap<String, u16>,
    reg_value: u16,
) {
    let mut variants: Vec<TokenStream> = Vec::new();
    let mut match_arms: Vec<TokenStream> = Vec::new();

    for (variant_name, value) in reg {
        let variant_ident = syn::Ident::new(
            &sanitize_identifier(&variant_name),
            proc_macro2::Span::call_site(),
        );
        variants.push(quote! {
            #variant_ident,
        });
        match_arms.push(quote! {
            #value => #enum_name::#variant_ident,
        });
    }

    // Add an Unknown variant
    variants.push(quote! {
        Unknown,
    });
    match_arms.push(quote! {
        _ => #enum_name::Unknown,
    });

    // Generate the enum and the From<Vec<u16>> implementation
    generated_enums.push(quote! {
        #[derive(Debug)]
        pub enum #enum_name {
            #(#variants)*
        }

        impl ModbusRegister<Vec<u16>> for #enum_name {
            fn reg() -> u16 { #reg_value }
        }

        impl From<Vec<u16>> for #enum_name {
            fn from(value: Vec<u16>) -> Self {
                match value[0] {
                    #(#match_arms)*
                }
            }
        }
    });
}
