use clap::Parser;
use log::{error, info};
use monmouse::errors::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

// fn setup_logger() {
//     env_logger::builder()
//         .filter_level(log::LevelFilter::Debug)
//         .init();
// }

fn main() -> Result<(), Error> {
    env_logger::builder().init();

    info!("monmouse-cli started");
    let result = monmouse::Eventloop::new(true).run();
    match &result {
        Ok(_) => info!("monmouse-cli ended normally"),
        Err(e) => error!("monmouse-cli ended with error: {}", e),
    }
    result
}
