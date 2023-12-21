use std::path::PathBuf;

use clap::Parser;
use log::{error, info};
use monmouse::errors::Error;
use monmouse::setting::{read_config, CONFIG_FILE_NAME};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

#[cfg(not(debug_assertions))]
pub const CLI_DEFAULT_CONFIG_DIR: &str = "conf";
#[cfg(debug_assertions)]
pub const CLI_DEFAULT_CONFIG_DIR: &str = "debug";

// fn setup_logger() {
//     env_logger::builder()
//         .filter_level(log::LevelFilter::Debug)
//         .init();
// }

fn main() -> Result<(), Error> {
    env_logger::builder().init();

    let file_path = PathBuf::from(CLI_DEFAULT_CONFIG_DIR).join(CONFIG_FILE_NAME);
    let config = read_config(file_path)?;

    info!("monmouse-cli started");
    let mut eventloop = monmouse::Eventloop::new(true);
    eventloop.load_config(config);
    let result = eventloop.run();
    match &result {
        Ok(_) => info!("monmouse-cli ended normally"),
        Err(e) => error!("monmouse-cli ended with error: {}", e),
    }
    result
}
