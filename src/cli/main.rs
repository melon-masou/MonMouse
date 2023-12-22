use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use log::{debug, error, info};
use monmouse::errors::Error;
use monmouse::message::GenericDevice;
use monmouse::setting::{read_config, CONFIG_FILE_NAME};
use monmouse::{POLL_MSGS, POLL_TIMEOUT};

#[cfg(not(debug_assertions))]
const CLI_DEFAULT_CONFIG_DIR: &str = "conf";
#[cfg(debug_assertions)]
const CLI_DEFAULT_CONFIG_DIR: &str = "debug";

fn default_config_file() -> String {
    PathBuf::from(CLI_DEFAULT_CONFIG_DIR)
        .join(CONFIG_FILE_NAME)
        .to_str()
        .unwrap()
        .to_owned()
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = default_config_file())]
    config_file: String,

    #[arg(short, long)]
    log_level: Option<String>,

    #[arg(short, long)]
    print_devices: bool,
}

fn setup_logger(o: Option<String>) -> Result<(), Error> {
    if let Some(log_level) = o {
        match log::LevelFilter::from_str(log_level.as_str()) {
            Ok(level) => env_logger::builder().filter_level(level).init(),
            Err(e) => return Err(Error::InvalidParam("log_level".to_owned(), e.to_string())),
        }
    } else {
        env_logger::builder().init()
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    setup_logger(args.log_level)?;
    let config = read_config(&PathBuf::from(args.config_file))?;
    debug!("Config loaded: {:?}", config);

    let mut eventloop = monmouse::Eventloop::new(true);

    if args.print_devices {
        let devices = eventloop.scan_devices()?;
        print_devices(devices);
        return Ok(());
    }

    eventloop.load_config(config);
    info!("monmouse-cli started");
    let result = eventloop.run();
    match &result {
        Ok(_) => info!("monmouse-cli ended normally"),
        Err(e) => error!("monmouse-cli ended with error: {}", e),
    }
    result
}

fn print_devices(devices: Vec<GenericDevice>) {
    for (i, d) in devices.iter().enumerate() {
        println!("Device[{}]", i);
        println!("ID: {}", d.id);
        println!("Type: {}", d.device_type);
        println!("Product: {}", d.product_name);
        println!("PlatformSpecificInfos:");
        for info in d.platform_specific_infos.iter() {
            println!("  {}: {}", info.0, info.1);
        }
        println!("----------------");
    }
}
