use crate::bitcoind::bitcoind_get_block_height;
use crate::config::Config;
use crate::db::cache::index_cache::IndexCache;
use crate::db::index::{index_block, roll_back_block};
use crate::{try_error, try_info};
use chainhook_sdk::chainhooks::bitcoin::{
    evaluate_bitcoin_chainhooks_on_chain_event, handle_bitcoin_hook_action,
    BitcoinChainhookOccurrence, BitcoinTriggerChainhook,
};
use chainhook_sdk::chainhooks::types::{BitcoinChainhookSpecification, BitcoinPredicateType};
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
use tokio_postgres::Client;

pub async fn drop_blocks(start_block: u64, end_block: u64, pg_client: &mut Client, ctx: &Context) {
    for block in start_block..=end_block {
        roll_back_block(pg_client, block, ctx).await;
    }
}

pub async fn scan_blocks(
    blocks: Vec<u64>,
    config: &Config,
    pg_client: &mut Client,
    index_cache: &mut IndexCache,
    ctx: &Context,
) -> Result<(), String> {
    let predicate = BitcoinChainhookSpecification {
        uuid: format!("runehook-internal-trigger"),
        owner_uuid: None,
        name: format!("runehook-internal-trigger"),
        network: config.event_observer.bitcoin_network.clone(),
        version: 1,
        blocks: Some(blocks),
        start_block: None,
        end_block: None,
        expired_at: None,
        expire_after_occurrence: None,
        predicate: BitcoinPredicateType::Block,
        action: chainhook_sdk::chainhooks::types::HookAction::Noop,
        include_proof: false,
        include_inputs: true,
        include_outputs: false,
        include_witness: false,
        enabled: true,
    };
    scan_bitcoin_chainstate_via_rpc_using_predicate(
        &predicate,
        &config,
        None,
        pg_client,
        index_cache,
        &ctx,
    )
    .await?;
    Ok(())
}

pub async fn scan_bitcoin_chainstate_via_rpc_using_predicate(
    predicate_spec: &BitcoinChainhookSpecification,
    config: &Config,
    event_observer_config_override: Option<&EventObserverConfig>,
    pg_client: &mut Client,
    index_cache: &mut IndexCache,
    ctx: &Context,
) -> Result<(), String> {
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
            None => (bitcoind_get_block_height(config, ctx), true),
        };
        floating_end_block = update_end_block;
        BlockHeights::BlockRange(start_block, end_block).get_sorted_entries()
    };

    let mut block_heights_to_scan =
        block_heights_to_scan_res.map_err(|_e| format!("Block start / end block spec invalid"))?;

    try_info!(
        ctx,
        "Scanning {} Bitcoin blocks",
        block_heights_to_scan.len()
    );
    let mut actions_triggered = 0;
    let mut _err_count = 0;

    let event_observer_config = match event_observer_config_override {
        Some(config_override) => config_override.clone(),
        None => config.event_observer.clone(),
    };
    let bitcoin_config = event_observer_config.get_bitcoin_config();
    let mut number_of_blocks_scanned = 0;
    let http_client = build_http_client();

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

        index_block(pg_client, index_cache, &mut block, ctx).await;

        match process_block_with_predicates(
            block,
            &vec![&predicate_spec],
            &event_observer_config,
            ctx,
        )
        .await
        {
            Ok(actions) => actions_triggered += actions,
            Err(_) => _err_count += 1,
        }

        // If we configured a "floating" end block, update the scan range with newer blocks that might have arrived to bitcoind.
        if block_heights_to_scan.is_empty() && floating_end_block {
            let bitcoind_tip = bitcoind_get_block_height(config, ctx);
            let new_tip = match predicate_spec.end_block {
                Some(end_block) => {
                    if end_block > bitcoind_tip {
                        bitcoind_tip
                    } else {
                        end_block
                    }
                }
                None => bitcoind_tip,
            };
            for entry in (current_block_height + 1)..new_tip {
                block_heights_to_scan.push_back(entry);
            }
        }
    }
    try_info!(
        ctx,
        "{number_of_blocks_scanned} blocks scanned, {actions_triggered} actions triggered"
    );

    Ok(())
}

async fn process_block_with_predicates(
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

async fn execute_predicates_action<'a>(
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
                try_error!(ctx, "unable to handle action {}", e);
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
