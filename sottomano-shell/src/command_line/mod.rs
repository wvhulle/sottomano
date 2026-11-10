use std::{error::Error, path::Path};

use clap::{Parser, Subcommand};

mod default_config;
mod secrets;
mod setup;

#[derive(Debug, Parser)]
#[command(
    name = "sottomano",
    version,
    about = "Desktop shell for niri, sway and Hyprland: two panels with hoverable readings.",
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
    #[command(about = "Set up the system pieces the widgets read on a distribution.")]
    Setup {
        #[command(subcommand)]
        distro: setup::DistroSetupCommand,
    },
}

impl SetupCommand {
    pub fn print(&self, config_home: &Path) -> Result<(), Box<dyn Error>> {
        match self {
            SetupCommand::DefaultConfig => default_config::print(),
            SetupCommand::Secrets => {
                secrets::print(config_home);
                Ok(())
            }
            SetupCommand::Setup { distro } => setup::run(distro),
        }
    }
}
