use std::path::Path;

use sottomano_toml_config::{
    SecretPaths,
    secrets::{SecretFile, SecretPresence},
};

pub fn print(config_home: &Path) {
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
