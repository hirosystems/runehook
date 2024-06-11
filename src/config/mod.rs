pub mod file;
pub mod generator;

use chainhook_sdk::observer::EventObserverConfig;

use file::ConfigFile;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Clone, Debug)]
pub struct Config {
    pub event_observer: EventObserverConfig,
}

impl Config {
    pub fn from_file_path(file_path: &str) -> Result<Config, String> {
        let file = File::open(file_path)
            .map_err(|e| format!("unable to read file {}\n{:?}", file_path, e))?;
        let mut file_reader = BufReader::new(file);
        let mut file_buffer = vec![];
        file_reader
            .read_to_end(&mut file_buffer)
            .map_err(|e| format!("unable to read file {}\n{:?}", file_path, e))?;

        let config_file: ConfigFile = match toml::from_slice(&file_buffer) {
            Ok(s) => s,
            Err(e) => {
                return Err(format!("Config file malformatted {}", e.to_string()));
            }
        };
        Config::from_config_file(config_file)
    }

    pub fn from_config_file(config_file: ConfigFile) -> Result<Config, String> {
        let event_observer =
            EventObserverConfig::new_using_overrides(config_file.network.as_ref())?;

        let config = Config { event_observer };
        Ok(config)
    }
}
