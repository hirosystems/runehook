use std::{collections::HashMap, num::NonZeroUsize, str::FromStr};

use bitcoin::{Network, ScriptBuf};
use chainhook_sdk::{types::bitcoin::TxIn, utils::Context};
use lru::LruCache;
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};
use tokio_postgres::{Client, Transaction};

use crate::{
    config::Config,
    db::{
        cache::utils::input_rune_balances_from_tx_inputs,
        models::{
            db_balance_change::DbBalanceChange, db_ledger_entry::DbLedgerEntry,
            db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
            db_supply_change::DbSupplyChange,
        },
        pg_get_max_rune_number, pg_get_rune_by_id, pg_get_rune_total_mints,
    },
    try_debug, try_info, try_warn,
};

use super::{
    db_cache::DbCache, input_rune_balance::InputRuneBalance, transaction_cache::TransactionCache,
    transaction_location::TransactionLocation, utils::move_block_output_cache_to_output_cache,
};

/// Holds rune data across multiple blocks for faster computations. Processes rune events as they happen during transactions and
/// generates database rows for later insertion.
pub struct IndexCache {
    pub network: Network,
    /// Number to be assigned to the next rune etching.
    next_rune_number: u32,
    /// LRU cache for runes.
    rune_cache: LruCache<RuneId, DbRune>,
    /// LRU cache for total mints for runes.
    rune_total_mints_cache: LruCache<RuneId, u128>,
    /// LRU cache for outputs with rune balances.
    output_cache: LruCache<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    /// Same as above but only for the current block. We use a `HashMap` instead of an LRU cache to make sure we keep all outputs
    /// in memory while we index this block. Must be cleared every time a new block is processed.
    block_output_cache: HashMap<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    /// Holds a single transaction's rune cache. Must be cleared every time a new transaction is processed.
    tx_cache: TransactionCache,
    /// Keeps rows that have not yet been inserted in the DB.
    pub db_cache: DbCache,
}

impl IndexCache {
    pub async fn new(config: &Config, pg_client: &mut Client, ctx: &Context) -> Self {
        let network = config.get_bitcoin_network();
        let cap = NonZeroUsize::new(config.resources.lru_cache_size).unwrap();
        IndexCache {
            network,
            next_rune_number: pg_get_max_rune_number(pg_client, ctx).await + 1,
            rune_cache: LruCache::new(cap),
            rune_total_mints_cache: LruCache::new(cap),
            output_cache: LruCache::new(cap),
            block_output_cache: HashMap::new(),
            tx_cache: TransactionCache::new(
                TransactionLocation {
                    network,
                    block_hash: "".to_string(),
                    block_height: 1,
                    timestamp: 0,
                    tx_index: 0,
                    tx_id: "".to_string(),
                },
                HashMap::new(),
                HashMap::new(),
                None,
                0,
            ),
            db_cache: DbCache::new(),
        }
    }

    pub async fn reset_max_rune_number(&mut self, db_tx: &mut Transaction<'_>, ctx: &Context) {
        self.next_rune_number = pg_get_max_rune_number(db_tx, ctx).await + 1;
    }

    /// Creates a fresh transaction index cache.
    pub async fn begin_transaction(
        &mut self,
        location: TransactionLocation,
        tx_inputs: &Vec<TxIn>,
        eligible_outputs: HashMap<u32, ScriptBuf>,
        first_eligible_output: Option<u32>,
        total_outputs: u32,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let input_runes = input_rune_balances_from_tx_inputs(
            tx_inputs,
            &self.block_output_cache,
            &mut self.output_cache,
            db_tx,
            ctx,
        )
        .await;
        #[cfg(not(feature = "release"))]
        {
            for (rune_id, balances) in input_runes.iter() {
                try_debug!(ctx, "INPUT {rune_id} {balances:?} {location}");
            }
            if input_runes.len() > 0 {
                try_debug!(
                    ctx,
                    "First output: {first_eligible_output:?}, total_outputs: {total_outputs}"
                );
            }
        }
        self.tx_cache = TransactionCache::new(
            location,
            input_runes,
            eligible_outputs,
            first_eligible_output,
            total_outputs,
        );
    }

    /// Finalizes the current transaction index cache by moving all unallocated balances to the correct output.
    pub fn end_transaction(&mut self, _db_tx: &mut Transaction<'_>, ctx: &Context) {
        let entries = self.tx_cache.allocate_remaining_balances(ctx);
        self.add_ledger_entries_to_db_cache(&entries);
    }

    pub fn end_block(&mut self) {
        move_block_output_cache_to_output_cache(
            &mut self.block_output_cache,
            &mut self.output_cache,
        );
    }

    pub async fn apply_runestone(
        &mut self,
        runestone: &Runestone,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        try_debug!(ctx, "{:?} {}", runestone, self.tx_cache.location);
        if let Some(new_pointer) = runestone.pointer {
            self.tx_cache.output_pointer = Some(new_pointer);
        }
    }

    pub async fn apply_cenotaph(
        &mut self,
        cenotaph: &Cenotaph,
        _db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        try_debug!(ctx, "{:?} {}", cenotaph, self.tx_cache.location);
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

    /// Take ledger entries returned by the `TransactionCache` and add them to the `DbCache`. Update global balances and counters
    /// as well.
    fn add_ledger_entries_to_db_cache(&mut self, entries: &Vec<DbLedgerEntry>) {
        self.db_cache.ledger_entries.extend(entries.clone());
        for entry in entries.iter() {
            match entry.operation {
                DbLedgerOperation::Etching => {
                    self.db_cache
                        .supply_changes
                        .entry(entry.rune_id.clone())
                        .and_modify(|i| {
                            i.total_operations += 1;
                        })
                        .or_insert(DbSupplyChange::from_operation(
                            entry.rune_id.clone(),
                            entry.block_height.clone(),
                        ));
                }
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
                        // Add to current block's output cache if it's received balance.
                        let k = (entry.tx_id.clone(), entry.output.unwrap().0);
                        let rune_id = RuneId::from_str(entry.rune_id.as_str()).unwrap();
                        let balance = InputRuneBalance {
                            address: entry.address.clone(),
                            amount: entry.amount.unwrap().0,
                        };
                        let mut default = HashMap::new();
                        default.insert(rune_id, vec![balance.clone()]);
                        self.block_output_cache
                            .entry(k)
                            .and_modify(|i| {
                                i.entry(rune_id)
                                    .and_modify(|v| v.push(balance.clone()))
                                    .or_insert(vec![balance]);
                            })
                            .or_insert(default);
                    }
                }
            }
        }
    }
}
