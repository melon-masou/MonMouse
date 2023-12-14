use clap::Parser;
use log::debug;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

fn setup_logger() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
}

fn main() {
    setup_logger();

    debug!("monmouse-cli started");
    debug!("monmouse-cli ended normally")
}
