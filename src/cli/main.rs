use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use log::{debug, error, info};
use monmouse::{
    errors::Error,
    message::setup_reactors,
    setting::{config_string, read_config, ProcessorSettings, Settings, CONFIG_FILE_NAME},
    SingleProcess,
};

#[cfg(not(debug_assertions))]
const CLI_DEFAULT_CONFIG_DIR: &str = ".";
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

    #[arg(long)]
    print_example_config: bool,
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
    if args.print_example_config {
        print!("{}", config_string(&Settings::example()).unwrap());
        return Ok(());
    }

    if args.print_devices {
        return print_devices();
    }

    setup_logger(args.log_level)?;
    let _single_process = SingleProcess::create()?;
    let reactors = setup_reactors(None);
    drop(reactors.ui);

    let config_path = PathBuf::from(args.config_file);
    let config = read_config(&config_path)?;
    debug!("Config loaded: {:?}", config);
    let poller = monmouse::Eventloop::new(true, reactors.control);

    info!("monmouse-cli started");
    let result = mouse_control_spawn(poller, &config.processor);
    match &result {
        Ok(_) => info!("monmouse-cli ended normally"),
        Err(e) => error!("monmouse-cli ended with error: {}", e),
    }

    result
}

fn mouse_control_spawn(
    mut poller: monmouse::Eventloop,
    settings: &ProcessorSettings,
) -> Result<(), Error> {
    poller.initialize()?;
    let result = (|| {
        poller.initialize_settings(settings)?;
        while poller.poll() {}
        Ok(())
    })();
    let cleanup = poller.terminate();
    result.and(cleanup)
}

fn print_devices() -> Result<(), Error> {
    let devices = monmouse::windows::list_pointer_devices()?;
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
    Ok(())
}
