use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    vetrata_shell::run_shell()
}
