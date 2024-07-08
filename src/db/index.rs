use std::collections::HashMap;

use bitcoin::absolute::LockTime;
use bitcoin::transaction::TxOut;
use bitcoin::Network;
use bitcoin::ScriptBuf;
use bitcoin::Transaction;
use chainhook_sdk::types::BitcoinTransactionData;
use chainhook_sdk::{types::BitcoinBlockData, utils::Context};
use ordinals::Artifact;
use ordinals::Runestone;
use tokio_postgres::Client;

use crate::db::cache::transaction_location::TransactionLocation;
use crate::db::pg_roll_back_block;
use crate::try_info;

use super::cache::index_cache::IndexCache;

pub fn get_rune_genesis_block_height(network: Network) -> u64 {
    match network {
        Network::Bitcoin => 840_000,
        Network::Testnet => todo!(),
        Network::Signet => todo!(),
        Network::Regtest => todo!(),
        _ => todo!(),
    }
}

/// Transforms a Bitcoin transaction from a Chainhook format to a rust bitcoin crate format so it can be parsed by the ord crate
/// to look for `Artifact`s. Also, takes all non-OP_RETURN outputs and returns them so they can be used later to receive runes.
fn bitcoin_tx_from_chainhook_tx(
    block: &BitcoinBlockData,
    tx: &BitcoinTransactionData,
) -> (Transaction, HashMap<u32, ScriptBuf>, Option<u32>, u32) {
    let mut outputs = vec![];
    let mut eligible_outputs = HashMap::new();
    let mut first_eligible_output: Option<u32> = None;
    for (i, output) in tx.metadata.outputs.iter().enumerate() {
        let script = ScriptBuf::from_bytes(output.get_script_pubkey_bytes());
        if !script.is_op_return() {
            eligible_outputs.insert(i as u32, script.clone());
            if first_eligible_output.is_none() {
                first_eligible_output = Some(i as u32);
            }
        }
        outputs.push(TxOut {
            value: output.value,
            script_pubkey: script,
        });
    }
    (
        Transaction {
            version: 2,
            lock_time: LockTime::from_time(block.timestamp).unwrap(),
            // Inputs don't matter for Runestone parsing.
            input: vec![],
            output: outputs,
        },
        eligible_outputs,
        first_eligible_output,
        tx.metadata.outputs.len() as u32,
    )
}

/// Index a Bitcoin block for runes data.
pub async fn index_block(
    pg_client: &mut Client,
    index_cache: &mut IndexCache,
    block: &mut BitcoinBlockData,
    ctx: &Context,
) {
    let stopwatch = std::time::Instant::now();
    let block_hash = &block.block_identifier.hash;
    let block_height = block.block_identifier.index;
    try_info!(ctx, "Indexing block {}...", block_height);

    let mut db_tx = pg_client
        .transaction()
        .await
        .expect("Unable to begin block processing pg transaction");
    index_cache.reset_max_rune_number(&mut db_tx, ctx).await;
    for tx in block.transactions.iter() {
        let (transaction, eligible_outputs, first_eligible_output, total_outputs) =
            bitcoin_tx_from_chainhook_tx(block, tx);
        let tx_index = tx.metadata.index;
        let tx_id = &tx.transaction_identifier.hash;
        let location = TransactionLocation {
            network: index_cache.network,
            block_hash: block_hash.clone(),
            block_height,
            tx_index,
            tx_id: tx_id.clone(),
            timestamp: block.timestamp,
        };
        index_cache
            .begin_transaction(
                location,
                &tx.metadata.inputs,
                eligible_outputs,
                first_eligible_output,
                total_outputs,
                &mut db_tx,
                ctx,
            )
            .await;
        if let Some(artifact) = Runestone::decipher(&transaction) {
            match artifact {
                Artifact::Runestone(runestone) => {
                    index_cache
                        .apply_runestone(&runestone, &mut db_tx, ctx)
                        .await;
                    if let Some(etching) = runestone.etching {
                        index_cache.apply_etching(&etching, &mut db_tx, ctx).await;
                    }
                    if let Some(mint_rune_id) = runestone.mint {
                        index_cache.apply_mint(&mint_rune_id, &mut db_tx, ctx).await;
                    }
                    for edict in runestone.edicts.iter() {
                        index_cache.apply_edict(edict, &mut db_tx, ctx).await;
                    }
                }
                Artifact::Cenotaph(cenotaph) => {
                    index_cache.apply_cenotaph(&cenotaph, &mut db_tx, ctx).await;
                    if let Some(etching) = cenotaph.etching {
                        index_cache
                            .apply_cenotaph_etching(&etching, &mut db_tx, ctx)
                            .await;
                    }
                    if let Some(mint_rune_id) = cenotaph.mint {
                        index_cache
                            .apply_cenotaph_mint(&mint_rune_id, &mut db_tx, ctx)
                            .await;
                    }
                }
            }
        }
        index_cache.end_transaction(&mut db_tx, ctx);
    }
    index_cache.end_block();
    index_cache.db_cache.flush(&mut db_tx, ctx).await;
    db_tx
        .commit()
        .await
        .expect("Unable to commit pg transaction");
    try_info!(
        ctx,
        "Block {} indexed in {}s",
        block_height,
        stopwatch.elapsed().as_millis() as f32 / 1000.0
    );
}

/// Roll back a Bitcoin block because of a re-org.
pub async fn roll_back_block(pg_client: &mut Client, block_height: u64, ctx: &Context) {
    let stopwatch = std::time::Instant::now();
    try_info!(ctx, "Rolling back block {}...", block_height);
    let mut db_tx = pg_client
        .transaction()
        .await
        .expect("Unable to begin block roll back pg transaction");
    pg_roll_back_block(block_height, &mut db_tx, ctx).await;
    db_tx
        .commit()
        .await
        .expect("Unable to commit pg transaction");
    try_info!(
        ctx,
        "Block {} rolled back in {}s",
        block_height,
        stopwatch.elapsed().as_millis() as f32 / 1000.0
    );
}
