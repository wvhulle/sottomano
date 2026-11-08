use std::{error::Error, path::Path};

use clap::{Parser, Subcommand};
use sottomano_toml_config::{
    SecretPaths, default_config_toml,
    secrets::{SecretFile, SecretPresence},
};

#[derive(Debug, Parser)]
#[command(
    name = "sottomano",
    version,
    about = "Desktop shell for the niri compositor: two panels with hoverable readings.",
    long_about = "Desktop shell for niri, sway and Hyprland: two panels with hoverable readings. \
                  Without a subcommand it starts the shell; the subcommands describe its setup \
                  instead."
)]
pub struct CommandLine {
    #[command(subcommand)]
    pub setup_command: Option<SetupCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SetupCommand {
    #[command(about = "Print every configuration key with its default value as TOML.")]
    DefaultConfig,
    #[command(
        about = "List the secret files the shell reads, what each is for and whether it is \
                 present."
    )]
    Secrets,
}

impl SetupCommand {
    pub fn print(&self, config_home: &Path) -> Result<(), Box<dyn Error>> {
        match self {
            SetupCommand::DefaultConfig => print!("{}", default_config_toml()?),
            SetupCommand::Secrets => print_secrets(config_home),
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
        let state = match present.presence_of(secret) {
            SecretPresence::Present => "present",
            SecretPresence::Absent => "absent",
        };
        println!("\n{} ({state})\n  {}", secret.file_name(), secret.purpose());
    }
}
