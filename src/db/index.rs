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

/// Transforms a Bitcoin transaction from a Chainhook format to a rust bitcoin format so it can be consumed by ord.
fn bitcoin_tx_from_chainhook_tx(
    block: &BitcoinBlockData,
    tx: &BitcoinTransactionData,
) -> Transaction {
    Transaction {
        version: 2,
        lock_time: LockTime::from_time(block.timestamp).unwrap(),
        // Inputs don't matter for Runestone parsing.
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
    }
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
        let transaction = bitcoin_tx_from_chainhook_tx(block, tx);
        let tx_index = tx.metadata.index;
        let tx_id = &tx.transaction_identifier.hash;
        index_cache
            .begin_transaction(block_hash, block_height, tx_index, tx_id, block.timestamp)
            .await;
        if let Some(artifact) = Runestone::decipher(&transaction) {
            match artifact {
                Artifact::Runestone(runestone) => {
                    index_cache
                        .apply_runestone(
                            &runestone,
                            &tx.metadata.inputs,
                            &tx.metadata.outputs,
                            &mut db_tx,
                            ctx,
                        )
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
                    index_cache
                        .apply_cenotaph(&cenotaph, &tx.metadata.inputs, &mut db_tx, ctx)
                        .await;
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
