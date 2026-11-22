use std::path::Path;

use sottomano_toml_config::{
    Config, ConfigFile, SecretPaths,
    secrets::{SecretFile, StoredSecret},
};

pub fn print(config_home: &Path) {
    let config = match ConfigFile::in_config_home(config_home).load() {
        Ok(loaded_config) => loaded_config,
        Err(load_error) => {
            eprintln!("the config file did not load, so no secrets are configured: {load_error}");
            Config::default()
        }
    };
    let discovered_paths = SecretPaths::discover(&config.secrets, config_home);
    println!(
        "Secret files are named in the [secrets] table of sottomano.toml; relative paths resolve \
         against {}",
        config_home.display()
    );
    for secret in SecretFile::ALL {
        let configured = config
            .secrets
            .configured_path(secret)
            .map_or_else(|| "not set".to_owned(), |path| path.display().to_string());
        let state = match discovered_paths.stored(secret) {
            StoredSecret::Present(_) => "present",
            StoredSecret::Absent => "absent",
        };
        println!(
            "\nsecrets.{} = {configured} ({state})\n  {}",
            secret.config_key(),
            secret.purpose()
        );
    }
}
