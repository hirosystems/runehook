use chainhook_sdk::{
    bitcoincore_rpc::{Auth, Client, RpcApi},
    utils::Context,
};

use crate::{config::Config, try_error};

fn get_client(config: &Config, ctx: &Context) -> Client {
    loop {
        let auth = Auth::UserPass(
            config.event_observer.bitcoind_rpc_username.clone(),
            config.event_observer.bitcoind_rpc_password.clone(),
        );
        match Client::new(&config.event_observer.bitcoind_rpc_url, auth) {
            Ok(con) => {
                return con;
            }
            Err(e) => {
                try_error!(ctx, "bitcoind unable to get client: {}", e.to_string());
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

pub fn bitcoind_get_block_height(config: &Config, ctx: &Context) -> u64 {
    let bitcoin_rpc = get_client(config, ctx);
    loop {
        match bitcoin_rpc.get_blockchain_info() {
            Ok(result) => {
                return result.blocks;
            }
            Err(e) => {
                try_error!(
                    ctx,
                    "bitcoind unable to get block height: {}",
                    e.to_string()
                );
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        };
    }
}
