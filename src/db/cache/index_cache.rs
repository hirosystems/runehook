use std::{collections::HashMap, num::NonZeroUsize, str::FromStr};

use bitcoin::Network;
use chainhook_sdk::{
    types::bitcoin::{TxIn, TxOut},
    utils::Context,
};
use lru::LruCache;
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};
use tokio_postgres::Transaction;

use crate::db::{
    get_output_rune_balances, get_rune_by_rune_id,
    models::{db_ledger_entry::DbLedgerEntry, db_rune::DbRune},
};

use super::{db_cache::DbCache, transaction_cache::TransactionCache};

/// Holds rune data across multiple blocks for faster computations. Processes rune events as they happen during transactions and
/// generates database rows for later insertion.
pub struct IndexCache {
    network: Network,
    next_rune_number: u32,
    runes: LruCache<RuneId, DbRune>,
    /// LRU cache for rune outputs. k = (tx_id, output), v = map(rune_id, amount)
    output_balances: LruCache<(String, u32), HashMap<RuneId, u128>>,
    /// Holds a single transaction's rune cache. Must be cleared every time a new transaction is processed.
    tx_cache: TransactionCache,
    pub db_cache: DbCache,
}

impl IndexCache {
    pub fn new(network: Network, lru_cache_size: usize, max_rune_number: u32) -> Self {
        let cap = NonZeroUsize::new(lru_cache_size).unwrap();
        IndexCache {
            network,
            next_rune_number: max_rune_number + 1,
            runes: LruCache::new(cap),
            output_balances: LruCache::new(cap),
            tx_cache: TransactionCache::new(network, 1, 0, &"".to_string(), 0),
            db_cache: DbCache::new(),
        }
    }

    /// Creates a fresh transaction index cache.
    pub async fn begin_transaction(
        &mut self,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) {
        self.tx_cache =
            TransactionCache::new(self.network, block_height, tx_index, tx_id, timestamp);
    }

    /// Finalizes the current transaction index cache.
    pub fn end_transaction(&mut self, _db_tx: &mut Transaction<'_>, ctx: &Context) {
        let entries = self.tx_cache.allocate_remaining_balances();
        for entry in entries.iter() {
            info!(
                ctx.expect_logger(),
                "Assign unallocated {} {} at block {}",
                entry.rune_id.clone(),
                entry.amount.0,
                entry.block_height.0
            );
        }
        self.add_ledger_entries_to_db_cache(entries);
    }

    pub async fn apply_runestone(
        &mut self,
        runestone: &Runestone,
        tx_inputs: &Vec<TxIn>,
        tx_outputs: &Vec<TxOut>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        info!(
            ctx.expect_logger(),
            "Runestone in tx {} ({}) at block {}",
            self.tx_cache.tx_id,
            self.tx_cache.tx_index,
            self.tx_cache.block_height
        );
        self.scan_tx_input_rune_balance(tx_inputs, db_tx, ctx).await;
        self.tx_cache.apply_runestone_pointer(runestone, tx_outputs);
    }

    pub async fn apply_cenotaph(
        &mut self,
        cenotaph: &Cenotaph,
        tx_inputs: &Vec<TxIn>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        info!(
            ctx.expect_logger(),
            "Cenotaph in tx {} ({}) at block {}",
            self.tx_cache.tx_id,
            self.tx_cache.tx_index,
            self.tx_cache.block_height
        );
        self.scan_tx_input_rune_balance(tx_inputs, db_tx, ctx).await;
        let entries = self.tx_cache.apply_cenotaph_input_burn(cenotaph);
        self.add_ledger_entries_to_db_cache(entries);
    }

    pub async fn apply_etching(
        &mut self,
        etching: &Etching,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let (rune_id, db_rune) = self.tx_cache.apply_etching(etching, self.next_rune_number);
        info!(
            ctx.expect_logger(),
            "Etching {} at block {}",
            db_rune.spaced_name.clone(),
            db_rune.block_height.0
        );
        self.db_cache.runes.push(db_rune.clone());
        self.runes.put(rune_id, db_rune);
        self.next_rune_number += 1;
    }

    pub async fn apply_cenotaph_etching(
        &mut self,
        rune: &Rune,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let (rune_id, db_rune) = self
            .tx_cache
            .apply_cenotaph_etching(rune, self.next_rune_number);
        info!(
            ctx.expect_logger(),
            "Etching cenotaph {} at block {}",
            db_rune.spaced_name.clone(),
            db_rune.block_height.0
        );
        self.db_cache.runes.push(db_rune.clone());
        self.runes.put(rune_id, db_rune);
        self.next_rune_number += 1;
    }

    pub async fn apply_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            // log
            return;
        };
        let ledger_entry = self.tx_cache.apply_mint(&rune_id, &db_rune);
        info!(
            ctx.expect_logger(),
            "Mint {} {} at block {}",
            db_rune.spaced_name.clone(),
            ledger_entry.amount.0,
            ledger_entry.block_height.0
        );
        self.add_ledger_entries_to_db_cache(vec![ledger_entry]);
    }

    pub async fn apply_cenotaph_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            // log
            return;
        };
        let ledger_entry = self.tx_cache.apply_cenotaph_mint(&rune_id, &db_rune);
        info!(
            ctx.expect_logger(),
            "Mint cenotaph {} {} at block {}",
            db_rune.spaced_name.clone(),
            ledger_entry.amount.0,
            ledger_entry.block_height.0
        );
        self.add_ledger_entries_to_db_cache(vec![ledger_entry]);
    }

    pub async fn apply_edict(&mut self, edict: &Edict, db_tx: &mut Transaction<'_>, ctx: &Context) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(&edict.id, db_tx, ctx).await else {
            // rune should exist?
            return;
        };
        let entries = self.tx_cache.apply_edict(edict, &db_rune);
        for entry in entries.iter() {
            info!(
                ctx.expect_logger(),
                "Edict {} {} at block {}",
                db_rune.spaced_name.clone(),
                entry.amount.0,
                entry.block_height.0
            );
        }
        self.add_ledger_entries_to_db_cache(entries);
    }

    async fn get_cached_rune_by_rune_id(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) -> Option<DbRune> {
        // Id 0:0 is used to mean the rune being etched in this transaction, if any.
        if rune_id.block == 0 && rune_id.tx == 0 {
            return self.tx_cache.etching.clone();
        }
        if let Some(cached_rune) = self.runes.get(&rune_id) {
            return Some(cached_rune.clone());
        }
        // Cache miss, look in DB.
        self.db_cache.flush(db_tx, ctx).await;
        let Some(db_rune) = get_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            return None;
        };
        self.runes.put(rune_id.clone(), db_rune.clone());
        return Some(db_rune);
    }

    async fn scan_tx_input_rune_balance(
        &mut self,
        tx_inputs: &Vec<TxIn>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        // Take all transaction inputs and transform them into rune balances to be allocated.
        let mut balances = HashMap::new();
        let mut cache_misses = vec![];
        for input in tx_inputs {
            let key = (
                input.previous_output.txid.hash[2..].to_string(),
                input.previous_output.vout,
            );
            // Look in the cache.
            if let Some(output_balance) = self.output_balances.get(&key) {
                for (k, v) in output_balance.iter() {
                    if let Some(balance) = balances.get(k) {
                        balances.insert(k.clone(), *v + *balance);
                    } else {
                        balances.insert(k.clone(), *v);
                    }
                }
            } else {
                cache_misses.push(key);
            }
        }
        if cache_misses.len() > 0 {
            // Look for misses in the DB
            self.db_cache.flush(db_tx, ctx).await;
            if let Some(output_balances) = get_output_rune_balances(cache_misses, db_tx, ctx).await
            {
                for (k, v) in output_balances.iter() {
                    if let Some(balance) = balances.get(k) {
                        balances.insert(k.clone(), *v + *balance);
                    } else {
                        balances.insert(k.clone(), *v);
                    }
                }
            }
        }
        self.tx_cache.unallocate_input_rune_balance(balances);
    }

    fn add_ledger_entries_to_db_cache(&mut self, entries: Vec<DbLedgerEntry>) {
        for entry in entries {
            // Add to output LRU cache.
            let k = (entry.tx_id.clone(), entry.output.0);
            let rune_id = RuneId::from_str(entry.rune_id.as_str()).unwrap();
            let amount = entry.amount.0;
            if let Some(v) = self.output_balances.get_mut(&k) {
                if let Some(rune_balance) = v.get(&rune_id) {
                    v.insert(rune_id, rune_balance + amount);
                } else {
                    v.insert(rune_id, amount);
                }
            } else {
                let mut v = HashMap::new();
                v.insert(rune_id, amount);
                self.output_balances.push(k, v);
            }
            // Send to DB cache.
            self.db_cache.ledger_entries.push(entry);
        }
    }
}
