use crate::config::Config;
use crate::db::cache::index_cache::IndexCache;
use crate::db::index::index_block;
use crate::db::{get_max_rune_number, init_db};
use chainhook_sdk::bitcoincore_rpc::RpcApi;
use chainhook_sdk::bitcoincore_rpc::{Auth, Client};
use chainhook_sdk::chainhooks::bitcoin::{
    evaluate_bitcoin_chainhooks_on_chain_event, handle_bitcoin_hook_action,
    BitcoinChainhookOccurrence, BitcoinTriggerChainhook,
};
use chainhook_sdk::chainhooks::types::BitcoinChainhookSpecification;
use chainhook_sdk::indexer::bitcoin::{
    build_http_client, download_and_parse_block_with_retry, retrieve_block_hash_with_retry,
    standardize_bitcoin_block,
};
use chainhook_sdk::observer::{gather_proofs, DataHandlerEvent, EventObserverConfig};
use chainhook_sdk::types::{
    BitcoinBlockData, BitcoinChainEvent, BitcoinChainUpdatedWithBlocksData,
};
use chainhook_sdk::utils::{file_append, send_request, BlockHeights, Context};
use std::collections::HashMap;

pub async fn scan_bitcoin_chainstate_via_rpc_using_predicate(
    predicate_spec: &BitcoinChainhookSpecification,
    config: &Config,
    event_observer_config_override: Option<&EventObserverConfig>,
    ctx: &Context,
) -> Result<(), String> {
    let auth = Auth::UserPass(
        config.event_observer.bitcoind_rpc_username.clone(),
        config.event_observer.bitcoind_rpc_username.clone(),
    );

    let bitcoin_rpc = match Client::new(&config.event_observer.bitcoind_rpc_url, auth) {
        Ok(con) => con,
        Err(message) => {
            return Err(format!("Bitcoin RPC error: {}", message.to_string()));
        }
    };
    let mut floating_end_block = false;

    let block_heights_to_scan_res = if let Some(ref blocks) = predicate_spec.blocks {
        BlockHeights::Blocks(blocks.clone()).get_sorted_entries()
    } else {
        let start_block = match predicate_spec.start_block {
            Some(start_block) => start_block,
            None => {
                return Err(
                    "Bitcoin chainhook specification must include a field start_block in replay mode"
                        .into(),
                );
            }
        };
        let (end_block, update_end_block) = match predicate_spec.end_block {
            Some(end_block) => (end_block, false),
            None => match bitcoin_rpc.get_blockchain_info() {
                Ok(result) => (result.blocks, true),
                Err(e) => {
                    return Err(format!(
                        "unable to retrieve Bitcoin chain tip ({})",
                        e.to_string()
                    ));
                }
            },
        };
        floating_end_block = update_end_block;
        BlockHeights::BlockRange(start_block, end_block).get_sorted_entries()
    };

    let mut block_heights_to_scan =
        block_heights_to_scan_res.map_err(|_e| format!("Block start / end block spec invalid"))?;

    info!(
        ctx.expect_logger(),
        "Starting predicate evaluation on {} Bitcoin blocks",
        block_heights_to_scan.len()
    );
    let mut actions_triggered = 0;
    let mut err_count = 0;

    let event_observer_config = match event_observer_config_override {
        Some(config_override) => config_override.clone(),
        None => config.event_observer.clone(),
    };
    let bitcoin_config = event_observer_config.get_bitcoin_config();
    let mut number_of_blocks_scanned = 0;
    let http_client = build_http_client();

    let mut pg_client = init_db(ctx).await.expect("Error initializing postgres db");

    let mut db_tx = pg_client
        .transaction()
        .await
        .expect("Error creating postgres transaction");
    let max_rune_number = get_max_rune_number(&mut db_tx, ctx).await;
    let mut index_cache = IndexCache::new(bitcoin::Network::Bitcoin, 5000, max_rune_number);
    let _ = db_tx.rollback().await;

    while let Some(current_block_height) = block_heights_to_scan.pop_front() {
        number_of_blocks_scanned += 1;

        let block_hash = retrieve_block_hash_with_retry(
            &http_client,
            &current_block_height,
            &bitcoin_config,
            ctx,
        )
        .await?;
        let raw_block =
            download_and_parse_block_with_retry(&http_client, &block_hash, &bitcoin_config, ctx)
                .await?;

        let mut block =
            standardize_bitcoin_block(raw_block, &config.event_observer.bitcoin_network, ctx)
                .unwrap();

        index_block(&mut pg_client, &mut index_cache, &mut block, ctx).await;

        match process_block_with_predicates(
            block,
            &vec![&predicate_spec],
            &event_observer_config,
            ctx,
        )
        .await
        {
            Ok(actions) => actions_triggered += actions,
            Err(_) => err_count += 1,
        }
    }
    info!(
        ctx.expect_logger(),
        "{number_of_blocks_scanned} blocks scanned, {actions_triggered} actions triggered"
    );

    Ok(())
}

pub async fn process_block_with_predicates(
    block: BitcoinBlockData,
    predicates: &Vec<&BitcoinChainhookSpecification>,
    event_observer_config: &EventObserverConfig,
    ctx: &Context,
) -> Result<u32, String> {
    let chain_event =
        BitcoinChainEvent::ChainUpdatedWithBlocks(BitcoinChainUpdatedWithBlocksData {
            new_blocks: vec![block],
            confirmed_blocks: vec![],
        });

    let (predicates_triggered, _predicates_evaluated, _) =
        evaluate_bitcoin_chainhooks_on_chain_event(&chain_event, predicates, ctx);

    execute_predicates_action(predicates_triggered, &event_observer_config, &ctx).await
}

pub async fn execute_predicates_action<'a>(
    hits: Vec<BitcoinTriggerChainhook<'a>>,
    config: &EventObserverConfig,
    ctx: &Context,
) -> Result<u32, String> {
    let mut actions_triggered = 0;
    let mut proofs = HashMap::new();
    for trigger in hits.into_iter() {
        if trigger.chainhook.include_proof {
            gather_proofs(&trigger, &mut proofs, &config, &ctx);
        }
        match handle_bitcoin_hook_action(trigger, &proofs) {
            Err(e) => {
                error!(ctx.expect_logger(), "unable to handle action {}", e);
            }
            Ok(action) => {
                actions_triggered += 1;
                match action {
                    BitcoinChainhookOccurrence::Http(request, _data) => {
                        send_request(request, 60, 3, &ctx).await?
                    }
                    BitcoinChainhookOccurrence::File(path, bytes) => {
                        file_append(path, bytes, &ctx)?
                    }
                    BitcoinChainhookOccurrence::Data(payload) => {
                        if let Some(ref tx) = config.data_handler_tx {
                            let _ = tx.send(DataHandlerEvent::Process(payload));
                        }
                    }
                };
            }
        }
    }

    Ok(actions_triggered)
}
