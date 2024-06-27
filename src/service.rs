use std::sync::mpsc::channel;

use crate::bitcoind::bitcoind_get_block_height;
use crate::config::Config;
use crate::db::cache::new_index_cache;
use crate::db::index::{get_rune_genesis_block_height, index_block};
use crate::db::{pg_connect, pg_get_block_height};
use crate::scan::bitcoin::scan_blocks;
use chainhook_sdk::observer::BitcoinBlockDataCached;
use chainhook_sdk::types::BlockIdentifier;
use chainhook_sdk::{
    observer::{start_event_observer, ObserverEvent, ObserverSidecar},
    types::BitcoinChainEvent,
    utils::Context,
};
use crossbeam_channel::select;

pub async fn start_service(config: &Config, ctx: &Context) -> Result<(), String> {
    let mut pg_client = pg_connect(&config, true, ctx).await;

    let (observer_cmd_tx, observer_cmd_rx) = channel();
    let (observer_event_tx, observer_event_rx) = crossbeam_channel::unbounded();
    let observer_sidecar = set_up_observer_sidecar_runloop(config, ctx)?;

    let mut index_cache = new_index_cache(config, &mut pg_client, ctx).await;
    let chain_tip = pg_get_block_height(&mut pg_client, ctx)
        .await
        .unwrap_or(get_rune_genesis_block_height(config.get_bitcoin_network()) - 1);
    loop {
        let bitcoind_chain_tip = bitcoind_get_block_height(config, ctx);
        if bitcoind_chain_tip < chain_tip {
            info!(
                ctx.expect_logger(),
                "Waiting for bitcoind to reach height {}, currently at {}",
                chain_tip,
                bitcoind_chain_tip
            );
            std::thread::sleep(std::time::Duration::from_secs(10));
        } else if bitcoind_chain_tip > chain_tip {
            info!(
                ctx.expect_logger(),
                "Scanning on block range {} to {}", chain_tip, bitcoind_chain_tip
            );
            scan_blocks(
                ((chain_tip + 1)..bitcoind_chain_tip).collect(),
                config,
                &mut pg_client,
                &mut index_cache,
                ctx,
            )
            .await?;
            break;
        }
    }

    // Start chainhook event observer, we're at chain tip.
    let event_observer_config = config.event_observer.clone();
    let context = if config.event_observer.display_logs {
        ctx.clone()
    } else {
        Context::empty()
    };
    let observer_cmd_tx_moved = observer_cmd_tx.clone();

    let _ = std::thread::spawn(move || {
        start_event_observer(
            event_observer_config,
            observer_cmd_tx_moved,
            observer_cmd_rx,
            Some(observer_event_tx),
            Some(observer_sidecar),
            None,
            context,
        )
        .expect("unable to start Stacks chain observer");
    });
    info!(ctx.expect_logger(), "Listening for new blocks",);

    loop {
        let event = match observer_event_rx.recv() {
            Ok(cmd) => cmd,
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error: broken channel {}",
                    e.to_string()
                );
                break;
            }
        };

        match event {
            ObserverEvent::BitcoinChainEvent((
                BitcoinChainEvent::ChainUpdatedWithBlocks(mut event),
                _,
            )) => {
                for block in event.new_blocks.iter_mut() {
                    index_block(&mut pg_client, &mut index_cache, block, ctx).await;
                }
            }
            ObserverEvent::BitcoinChainEvent((
                BitcoinChainEvent::ChainUpdatedWithReorg(mut event),
                _,
            )) => {
                for block in event.blocks_to_rollback.iter() {
                    // rollback
                }
                for block in event.blocks_to_apply.iter() {
                    // apply
                }
            }
            ObserverEvent::Terminate => {}
            _ => {}
        }
    }
    Ok(())
}

pub fn set_up_observer_sidecar_runloop(
    config: &Config,
    ctx: &Context,
) -> Result<ObserverSidecar, String> {
    // Sidecar will be receiving blocks to mutate
    let (block_mutator_in_tx, block_mutator_in_rx) = crossbeam_channel::unbounded();
    // Sidecar will be sending mutated blocks back to chainhook-sdk
    let (block_mutator_out_tx, block_mutator_out_rx) = crossbeam_channel::unbounded();
    // HandleBlock
    let (chain_event_notifier_tx, chain_event_notifier_rx) = crossbeam_channel::unbounded();
    let observer_sidecar = ObserverSidecar {
        bitcoin_blocks_mutator: Some((block_mutator_in_tx, block_mutator_out_rx)),
        bitcoin_chain_event_notifier: Some(chain_event_notifier_tx),
    };
    let ctx = ctx.clone();
    let config = config.clone();

    let _ = hiro_system_kit::thread_named("Observer Sidecar Runloop").spawn(move || loop {
        select! {
            recv(block_mutator_in_rx) -> msg => {
                if let Ok((mut blocks_to_mutate, blocks_ids_to_rollback)) = msg {
                    chainhook_sidecar_mutate_blocks(
                        &mut blocks_to_mutate,
                        &blocks_ids_to_rollback,
                        &config,
                        &ctx,
                    );
                    let _ = block_mutator_out_tx.send(blocks_to_mutate);
                }
            }
            recv(chain_event_notifier_rx) -> msg => {
                if let Ok(_command) = msg {
                    //
                }
            }
        }
    });

    Ok(observer_sidecar)
}

pub fn chainhook_sidecar_mutate_blocks(
    blocks_to_mutate: &mut Vec<BitcoinBlockDataCached>,
    blocks_ids_to_rollback: &Vec<BlockIdentifier>,
    _config: &Config,
    _ctx: &Context,
) {
    for _block_id_to_rollback in blocks_ids_to_rollback.iter() {
        // Delete local caches
    }

    for cache in blocks_to_mutate.iter_mut() {
        if cache.processed_by_sidecar {
            // Update data
        } else {
            // Process data
            // ...
            // Block 840,000
            // handle_block_processing(&mut cache.block, ctx);
            cache.processed_by_sidecar = true;
        }
    }
}
