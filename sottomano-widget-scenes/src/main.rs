use std::error::Error;

use sottomano_toml_config::Config;
use sottomano_widget_scenes::showcase_program::ShowcaseProgram;

fn main() -> Result<(), Box<dyn Error>> {
    let program = ShowcaseProgram::interactive(&Config::default());
    Ok(iced_winit::run(iced_tester::attach(program))?)
}
