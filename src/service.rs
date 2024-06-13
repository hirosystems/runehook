use std::sync::mpsc::channel;

use crate::config::Config;
use crate::db::index_cache::IndexCache;
use crate::db::init_db;
use bitcoin::absolute::LockTime;
use bitcoin::transaction::TxOut;
use bitcoin::ScriptBuf;
use bitcoin::Transaction;
use chainhook_sdk::observer::BitcoinBlockDataCached;
use chainhook_sdk::types::BitcoinBlockData;
use chainhook_sdk::types::BlockIdentifier;
use chainhook_sdk::{
    observer::{start_event_observer, ObserverEvent, ObserverSidecar},
    types::BitcoinChainEvent,
    utils::Context,
};
use crossbeam_channel::select;
use ordinals::Artifact;
use ordinals::Runestone;
use tokio_postgres::NoTls;

pub async fn start_service(config: &Config, ctx: &Context) -> Result<(), String> {
    let ctx_moved = ctx.clone();
    let _init_db_res = match init_db(ctx).await {
        Ok(res) => res,
        Err(e) => {
            error!(
                ctx_moved.expect_logger(),
                "Init DB error: {}",
                e.to_string(),
            );
            std::process::exit(1);
        }
    };

    // let _init_db_res = match init_db() {
    //     Ok(res) => res,
    //     Err(e) => {
    //         error!(
    //             ctx.expect_logger(),
    //             "Init DB error: {}", e.to_string(),
    //         );
    //         std::process::exit(1);
    //     }
    // };

    let (observer_cmd_tx, observer_cmd_rx) = channel();
    let (observer_event_tx, observer_event_rx) = crossbeam_channel::unbounded();

    let observer_sidecar = set_up_observer_sidecar_runloop(config, ctx)?;

    // Start chainhook event observer
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

    let context_cloned = ctx.clone();
    let config_cloned = config.clone();

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
                BitcoinChainEvent::ChainUpdatedWithBlocks(blocks),
                _,
            )) => {}
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
                if let Ok(command) = msg {
                    //
                }
            }
        }
    });

    Ok(observer_sidecar)
}

pub async fn handle_block_processing(block: &mut BitcoinBlockData, ctx: &Context) {
    let (mut pg_client, connection) =
        tokio_postgres::connect("host=localhost user=postgres", NoTls)
            .await
            .expect("unable to create pg client");

    let mut db_tx = pg_client
        .transaction()
        .await
        .expect("unable to begin pg transaction");
    // TODO: Create outside of this fn
    let mut memory_cache = IndexCache::new();
    for tx in block.transactions.iter() {
        let transaction = Transaction {
            version: 2,
            lock_time: LockTime::from_time(block.timestamp).unwrap(),
            input: vec![],
            output: tx
                .metadata
                .outputs
                .iter()
                .map(|output| TxOut {
                    value: output.value,
                    script_pubkey: ScriptBuf::from_bytes(output.get_script_pubkey_bytes()),
                })
                .collect(),
        };
        let runestone_opt = Runestone::decipher(&transaction);

        if let Some(artifact) = runestone_opt {
            match artifact {
                Artifact::Runestone(runestone) => {
                    ctx.try_log(|logger| {
                        info!(
                            logger,
                            "Block #{} - detected runestone {:?}",
                            block.block_identifier.index,
                            runestone
                        )
                    });
                    if let Some(etching) = runestone.etching {
                        memory_cache.insert_etching(
                            &etching,
                            block.block_identifier.index,
                            tx.metadata.index,
                            &tx.transaction_identifier.hash,
                            &mut db_tx,
                            ctx,
                        ).await;
                    }
                    for edict in runestone.edicts.iter() {
                        // if let Some(pointer) = runestone.pointer {
                        //     let output = &tx.metadata.outputs[pointer as usize];
                        //     let bytes = hex::decode(&output.script_pubkey).unwrap();
                        //     let script = ScriptBuf::from_bytes(bytes);
                        //     script.is_op_return();
                        // }
                        // for vout in tx.metadata.outputs.iter().fil
                        memory_cache.insert_edict(
                            edict,
                            block.block_identifier.index,
                            tx.metadata.index,
                            &tx.transaction_identifier.hash,
                            &"test_sender".to_string(),
                            &"test_receiver".to_string(),
                            &mut db_tx,
                            ctx,
                        ).await;
                    }
                }
                Artifact::Cenotaph(data) => {
                    ctx.try_log(|logger| {
                        info!(
                            logger,
                            "Block #{} - detected cenotaph {:?}",
                            block.block_identifier.index,
                            data
                        )
                    });
                }
            }
        }
    }
    memory_cache.db_cache.flush(&mut db_tx, ctx);
    let _ = db_tx.commit();
}

pub fn chainhook_sidecar_mutate_blocks(
    blocks_to_mutate: &mut Vec<BitcoinBlockDataCached>,
    blocks_ids_to_rollback: &Vec<BlockIdentifier>,
    config: &Config,
    ctx: &Context,
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
