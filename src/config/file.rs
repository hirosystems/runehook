use chainhook_sdk::observer::EventObserverConfigOverrides;

use super::Config;

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigFile {
    pub network: Option<EventObserverConfigOverrides>,
    pub postgres: PostgresConfigFile,
}

impl ConfigFile {
    pub fn from_file_path(file_path: &str) -> Result<ConfigFile, String> {
        unimplemented!()
    }

    pub fn from_config_file(config_file: ConfigFile) -> Result<Config, String> {
        unimplemented!()
    }

    pub fn default(
        devnet: bool,
        testnet: bool,
        mainnet: bool,
        config_path: &Option<String>,
    ) -> Result<Config, String> {
        unimplemented!()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfigFile {
    pub runes_internals: Option<bool>,
    pub chainhook_internals: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PostgresConfigFile {
    pub database: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PredicatesApiConfigFile {
    pub http_port: Option<u16>,
    pub database_uri: Option<String>,
    pub display_logs: Option<bool>,
    pub disabled: Option<bool>,
}

// #[derive(Deserialize, Debug, Clone)]
// pub struct SnapshotConfigFile {
//     pub download_url: Option<String>,
// }

// #[derive(Deserialize, Debug, Clone)]
// pub struct ResourcesConfigFile {
//     pub ulimit: Option<usize>,
//     pub cpu_core_available: Option<usize>,
//     pub memory_available: Option<usize>,
//     pub bitcoind_rpc_threads: Option<usize>,
//     pub bitcoind_rpc_timeout: Option<u32>,
//     pub expected_observers_count: Option<usize>,
//     pub brc20_lru_cache_size: Option<usize>,
// }

// #[derive(Deserialize, Debug, Clone)]
// pub struct NetworkConfigFile {
//     pub mode: String,
//     pub bitcoind_rpc_url: String,
//     pub bitcoind_rpc_username: String,
//     pub bitcoind_rpc_password: String,
//     pub bitcoind_zmq_url: Option<String>,
//     pub stacks_node_rpc_url: Option<String>,
//     pub stacks_events_ingestion_port: Option<u16>,
// }
