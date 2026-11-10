use std::error::Error;

use sottomano_toml_config::default_config_toml;

pub fn print() -> Result<(), Box<dyn Error>> {
    std::print!("{}", default_config_toml()?);
    Ok(())
}
