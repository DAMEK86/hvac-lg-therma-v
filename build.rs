use std::fs;
use std::io::Write;

fn main() {
    let code = modbus_mapping::generate_registers("./registers.json");
    let syntax_tree = syn::parse2(code).unwrap();
    let formatted = prettyplease::unparse(&syntax_tree);

    let mut outfile = fs::File::create("./src/registers.rs").unwrap();
    outfile.write_all(formatted.as_bytes()).unwrap();
}
