use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroUsize,
    str::FromStr,
};

use bitcoin::Network;
use chainhook_sdk::{
    types::bitcoin::{TxIn, TxOut},
    utils::Context,
};
use lru::LruCache;
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};
use tokio_postgres::Transaction;

use crate::{
    db::{
        models::{
            db_balance_change::DbBalanceChange, db_ledger_entry::DbLedgerEntry,
            db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
            db_supply_change::DbSupplyChange,
        },
        pg_get_missed_input_rune_balances, pg_get_rune_by_id, pg_get_rune_total_mints,
    },
    try_debug, try_info, try_warn,
};

use super::{
    db_cache::DbCache,
    transaction_cache::{InputRuneBalance, TransactionCache},
};

/// Holds rune data across multiple blocks for faster computations. Processes rune events as they happen during transactions and
/// generates database rows for later insertion.
pub struct IndexCache {
    network: Network,
    /// Number to be assigned to the next rune etching.
    next_rune_number: u32,
    /// LRU cache for runes.
    rune_cache: LruCache<RuneId, DbRune>,
    /// LRU cache for total mints for runes.
    rune_total_mints_cache: LruCache<RuneId, u128>,
    /// LRU cache for outputs with rune balances.
    output_cache: LruCache<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    /// Holds a single transaction's rune cache. Must be cleared every time a new transaction is processed.
    tx_cache: TransactionCache,
    /// Keeps rows that have not yet been inserted in the DB.
    pub db_cache: DbCache,
}

impl IndexCache {
    pub fn new(network: Network, lru_cache_size: usize, max_rune_number: u32) -> Self {
        let cap = NonZeroUsize::new(lru_cache_size).unwrap();
        IndexCache {
            network,
            next_rune_number: max_rune_number + 1,
            rune_cache: LruCache::new(cap),
            rune_total_mints_cache: LruCache::new(cap),
            output_cache: LruCache::new(cap),
            tx_cache: TransactionCache::new(network, &"".to_string(), 1, 0, &"".to_string(), 0),
            db_cache: DbCache::new(),
        }
    }

    /// Creates a fresh transaction index cache.
    pub async fn begin_transaction(
        &mut self,
        block_hash: &String,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) {
        self.tx_cache = TransactionCache::new(
            self.network,
            block_hash,
            block_height,
            tx_index,
            tx_id,
            timestamp,
        );
    }

    /// Finalizes the current transaction index cache.
    pub fn end_transaction(&mut self, _db_tx: &mut Transaction<'_>, ctx: &Context) {
        let entries = self.tx_cache.allocate_remaining_balances(ctx);
        self.add_ledger_entries_to_db_cache(&entries);
    }

    pub async fn apply_runestone(
        &mut self,
        runestone: &Runestone,
        tx_inputs: &Vec<TxIn>,
        tx_outputs: &Vec<TxOut>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        try_debug!(ctx, "{:?} {}", runestone, self.tx_cache.location);
        self.scan_tx_input_rune_balance(tx_inputs, db_tx, ctx).await;
        self.tx_cache
            .apply_runestone_pointer(runestone, tx_outputs, ctx);
    }

    pub async fn apply_cenotaph(
        &mut self,
        cenotaph: &Cenotaph,
        tx_inputs: &Vec<TxIn>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        try_debug!(ctx, "{:?} {}", cenotaph, self.tx_cache.location);
        self.scan_tx_input_rune_balance(tx_inputs, db_tx, ctx).await;
        let entries = self.tx_cache.apply_cenotaph_input_burn(cenotaph);
        self.add_ledger_entries_to_db_cache(&entries);
    }

    pub async fn apply_etching(
        &mut self,
        etching: &Etching,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let (rune_id, db_rune, entry) = self.tx_cache.apply_etching(etching, self.next_rune_number);
        try_info!(
            ctx,
            "Etching {} ({}) {}",
            db_rune.spaced_name,
            db_rune.id,
            self.tx_cache.location
        );
        self.db_cache.runes.push(db_rune.clone());
        self.rune_cache.put(rune_id, db_rune);
        self.add_ledger_entries_to_db_cache(&vec![entry]);
        self.next_rune_number += 1;
    }

    pub async fn apply_cenotaph_etching(
        &mut self,
        rune: &Rune,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let (rune_id, db_rune, entry) = self
            .tx_cache
            .apply_cenotaph_etching(rune, self.next_rune_number);
        try_info!(
            ctx,
            "Etching cenotaph {} ({}) {}",
            db_rune.spaced_name,
            db_rune.id,
            self.tx_cache.location
        );
        self.db_cache.runes.push(db_rune.clone());
        self.rune_cache.put(rune_id, db_rune);
        self.add_ledger_entries_to_db_cache(&vec![entry]);
        self.next_rune_number += 1;
    }

    pub async fn apply_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            try_warn!(
                ctx,
                "Rune {} not found for mint {}",
                rune_id,
                self.tx_cache.location
            );
            return;
        };
        let total_mints = self
            .get_cached_rune_total_mints(rune_id, db_tx, ctx)
            .await
            .unwrap_or(0);
        if let Some(ledger_entry) = self
            .tx_cache
            .apply_mint(&rune_id, total_mints, &db_rune, ctx)
        {
            self.add_ledger_entries_to_db_cache(&vec![ledger_entry.clone()]);
            if let Some(total) = self.rune_total_mints_cache.get_mut(rune_id) {
                *total += 1;
            } else {
                self.rune_total_mints_cache.put(rune_id.clone(), 1);
            }
        }
    }

    pub async fn apply_cenotaph_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            try_warn!(
                ctx,
                "Rune {} not found for cenotaph mint {}",
                rune_id,
                self.tx_cache.location
            );
            return;
        };
        let total_mints = self
            .get_cached_rune_total_mints(rune_id, db_tx, ctx)
            .await
            .unwrap_or(0);
        if let Some(ledger_entry) =
            self.tx_cache
                .apply_cenotaph_mint(&rune_id, total_mints, &db_rune, ctx)
        {
            self.add_ledger_entries_to_db_cache(&vec![ledger_entry]);
            if let Some(total) = self.rune_total_mints_cache.get_mut(rune_id) {
                *total += 1;
            } else {
                self.rune_total_mints_cache.put(rune_id.clone(), 1);
            }
        }
    }

    pub async fn apply_edict(&mut self, edict: &Edict, db_tx: &mut Transaction<'_>, ctx: &Context) {
        let Some(db_rune) = self.get_cached_rune_by_rune_id(&edict.id, db_tx, ctx).await else {
            try_warn!(
                ctx,
                "Rune {} not found for edict {}",
                edict.id,
                self.tx_cache.location
            );
            return;
        };
        let entries = self.tx_cache.apply_edict(edict, ctx);
        for entry in entries.iter() {
            try_info!(
                ctx,
                "Edict {} {} {}",
                db_rune.spaced_name,
                entry.amount.unwrap().0,
                self.tx_cache.location
            );
        }
        self.add_ledger_entries_to_db_cache(&entries);
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
        if let Some(cached_rune) = self.rune_cache.get(&rune_id) {
            return Some(cached_rune.clone());
        }
        // Cache miss, look in DB.
        self.db_cache.flush(db_tx, ctx).await;
        let Some(db_rune) = pg_get_rune_by_id(rune_id, db_tx, ctx).await else {
            return None;
        };
        self.rune_cache.put(rune_id.clone(), db_rune.clone());
        return Some(db_rune);
    }

    async fn get_cached_rune_total_mints(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) -> Option<u128> {
        let real_rune_id = if rune_id.block == 0 && rune_id.tx == 0 {
            let Some(etching) = self.tx_cache.etching.as_ref() else {
                return None;
            };
            RuneId::from_str(etching.id.as_str()).unwrap()
        } else {
            rune_id.clone()
        };
        if let Some(total) = self.rune_total_mints_cache.get(&real_rune_id) {
            return Some(*total);
        }
        // Cache miss, look in DB.
        self.db_cache.flush(db_tx, ctx).await;
        let Some(total) = pg_get_rune_total_mints(rune_id, db_tx, ctx).await else {
            return None;
        };
        self.rune_total_mints_cache.put(rune_id.clone(), total);
        return Some(total);
    }

    /// Takes all transaction inputs and transform them into rune balances to be allocated.
    async fn scan_tx_input_rune_balance(
        &mut self,
        tx_inputs: &Vec<TxIn>,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        // Maps input index to all of its rune balances. Useful in order to keep rune inputs in order.
        let mut indexed_input_runes = HashMap::new();

        // Look in memory cache.
        let mut cache_misses = vec![];
        for (i, input) in tx_inputs.iter().enumerate() {
            let tx_id = input.previous_output.txid.hash[2..].to_string();
            let vout = input.previous_output.vout;
            if let Some(map) = self.output_cache.get(&(tx_id.clone(), vout)) {
                indexed_input_runes.insert(i as u32, map.clone());
            } else {
                cache_misses.push((i as u32, tx_id, vout));
            }
        }

        // Look for misses in database.
        if cache_misses.len() > 0 {
            // self.db_cache.flush(db_tx, ctx).await;
            let output_balances = pg_get_missed_input_rune_balances(cache_misses, db_tx, ctx).await;
            indexed_input_runes.extend(output_balances);
        }

        let mut final_input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>> = HashMap::new();
        let mut input_keys: Vec<u32> = indexed_input_runes.keys().copied().collect();
        input_keys.sort();
        for key in input_keys.iter() {
            let input_value = indexed_input_runes.get(key).unwrap();
            for (rune_id, vec) in input_value.iter() {
                if let Some(rune) = final_input_runes.get_mut(rune_id) {
                    rune.extend(vec.clone());
                } else {
                    final_input_runes.insert(*rune_id, VecDeque::from(vec.clone()));
                }
            }
        }

        self.tx_cache
            .set_input_rune_balances(final_input_runes, ctx);
    }

    /// Take ledger entries returned by the `TransactionCache` and add them to the `DbCache`. Update global balances and counters
    /// as well.
    fn add_ledger_entries_to_db_cache(&mut self, entries: &Vec<DbLedgerEntry>) {
        self.db_cache.ledger_entries.extend(entries.clone());
        for entry in entries.iter() {
            match entry.operation {
                DbLedgerOperation::Etching => {}
                DbLedgerOperation::Mint => {
                    self.db_cache
                        .supply_changes
                        .entry(entry.rune_id.clone())
                        .and_modify(|i| {
                            i.minted += entry.amount.unwrap();
                            i.total_mints += 1;
                            i.total_operations += 1;
                        })
                        .or_insert(DbSupplyChange::from_mint(
                            entry.rune_id.clone(),
                            entry.block_height.clone(),
                            entry.amount.unwrap(),
                        ));
                }
                DbLedgerOperation::Burn => {
                    self.db_cache
                        .supply_changes
                        .entry(entry.rune_id.clone())
                        .and_modify(|i| {
                            i.burned += entry.amount.unwrap();
                            i.total_burns += 1;
                            i.total_operations += 1;
                        })
                        .or_insert(DbSupplyChange::from_burn(
                            entry.rune_id.clone(),
                            entry.block_height.clone(),
                            entry.amount.unwrap(),
                        ));
                }
                DbLedgerOperation::Send => {
                    self.db_cache
                        .supply_changes
                        .entry(entry.rune_id.clone())
                        .and_modify(|i| i.total_operations += 1)
                        .or_insert(DbSupplyChange::from_operation(
                            entry.rune_id.clone(),
                            entry.block_height.clone(),
                        ));
                    if let Some(address) = entry.address.clone() {
                        self.db_cache
                            .balance_deductions
                            .entry((entry.rune_id.clone(), address.clone()))
                            .and_modify(|i| i.balance += entry.amount.unwrap())
                            .or_insert(DbBalanceChange::from_operation(
                                entry.rune_id.clone(),
                                entry.block_height.clone(),
                                address,
                                entry.amount.unwrap(),
                            ));
                    }
                }
                DbLedgerOperation::Receive => {
                    self.db_cache
                        .supply_changes
                        .entry(entry.rune_id.clone())
                        .and_modify(|i| i.total_operations += 1)
                        .or_insert(DbSupplyChange::from_operation(
                            entry.rune_id.clone(),
                            entry.block_height.clone(),
                        ));
                    if let Some(address) = entry.address.clone() {
                        self.db_cache
                            .balance_increases
                            .entry((entry.rune_id.clone(), address.clone()))
                            .and_modify(|i| i.balance += entry.amount.unwrap())
                            .or_insert(DbBalanceChange::from_operation(
                                entry.rune_id.clone(),
                                entry.block_height.clone(),
                                address,
                                entry.amount.unwrap(),
                            ));
                    }

                    // Add to output LRU cache if it's received balance.
                    let k = (entry.tx_id.clone(), entry.output.unwrap().0);
                    let rune_id = RuneId::from_str(entry.rune_id.as_str()).unwrap();
                    let balance = InputRuneBalance {
                        address: entry.address.clone(),
                        amount: entry.amount.unwrap().0,
                    };
                    if let Some(v) = self.output_cache.get_mut(&k) {
                        if let Some(rune_balance) = v.get_mut(&rune_id) {
                            rune_balance.push(balance);
                        } else {
                            v.insert(rune_id, vec![balance]);
                        }
                    } else {
                        let mut v = HashMap::new();
                        v.insert(rune_id, vec![balance]);
                        self.output_cache.push(k, v);
                    }
                }
            }
        }
    }
}
