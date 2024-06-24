pub mod file;
pub mod generator;

use chainhook_sdk::observer::EventObserverConfig;

use file::ConfigFile;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Clone, Debug)]
pub struct PostgresConfig {
    pub database: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub event_observer: EventObserverConfig,
    pub postgres: PostgresConfig,
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

        let config = Config {
            event_observer,
            postgres: PostgresConfig {
                database: config_file
                    .postgres
                    .database
                    .unwrap_or("postgres".to_string()),
                host: config_file.postgres.host.unwrap_or("localhost".to_string()),
                port: config_file.postgres.port.unwrap_or(5432),
                username: config_file
                    .postgres
                    .username
                    .unwrap_or("postgres".to_string()),
                password: config_file.postgres.password,
            },
        };
        Ok(config)
    }
}
