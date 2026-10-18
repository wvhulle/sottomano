use std::{error::Error, path::Path};

use clap::Parser;
use sottomano_config::{SecretPaths, default_config_toml, secrets::SecretFile};

#[derive(Debug, Parser)]
#[command(
    name = "sottomano",
    version,
    about = "Desktop shell for the niri compositor: two panels with hoverable readings.",
    long_about = "Desktop shell for the niri compositor: two panels with hoverable readings. \
                  Without options it starts the shell; the options below describe its setup \
                  instead."
)]
pub struct CommandLine {
    #[arg(
        long,
        help = "Print every configuration key with its default value as TOML, then exit."
    )]
    pub print_default_config: bool,

    #[arg(
        long,
        help = "List the secret files the shell reads, what each is for and whether it is \
                present, then exit."
    )]
    pub list_secrets: bool,
}

impl CommandLine {
    pub fn describes_setup(&self) -> bool {
        self.print_default_config || self.list_secrets
    }

    pub fn print_setup(&self, config_home: &Path) -> Result<(), Box<dyn Error>> {
        if self.print_default_config {
            print!("{}", default_config_toml()?);
        }
        if self.list_secrets {
            print_secrets(config_home);
        }
        Ok(())
    }
}

fn print_secrets(config_home: &Path) {
    let present = SecretPaths::discover(config_home);
    println!(
        "Secret files are read from {}",
        SecretFile::directory(config_home).display()
    );
    for secret in SecretFile::ALL {
        let state = if present.contains(secret) {
            "present"
        } else {
            "absent"
        };
        println!("\n{} ({state})\n  {}", secret.file_name(), secret.purpose());
    }
}
