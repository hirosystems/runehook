use std::num::NonZeroUsize;

use chainhook_sdk::{types::bitcoin::TxIn, utils::Context};
use lru::LruCache;
use ordinals::{Edict, Etching, Rune, RuneId};
use tokio_postgres::Transaction;

use crate::db::{get_rune_by_rune_id, models::DbRune};

use super::{db_cache::DbCache, transaction_cache::TransactionCache};

/// Holds rune data across multiple blocks for faster computations. Processes rune events as they happen during transactions and
/// generates database rows for later insertion.
pub struct IndexCache {
    next_rune_number: u32,
    runes: LruCache<RuneId, DbRune>,
    /// Holds a single transaction's rune cache. Must be cleared every time a new transaction is processed.
    pub tx_cache: TransactionCache,
    pub db_cache: DbCache,
}

impl IndexCache {
    pub fn new(lru_cache_size: usize, max_rune_number: u32) -> Self {
        let cap = NonZeroUsize::new(lru_cache_size).unwrap();
        IndexCache {
            next_rune_number: max_rune_number + 1,
            runes: LruCache::new(cap),
            tx_cache: TransactionCache::new(1, 0, &"".to_string()),
            db_cache: DbCache::new(),
        }
    }

    /// Creates a fresh transaction index cache.
    pub fn begin_transaction(&mut self, block_height: u64, tx_index: u32, tx_id: &String, tx_inputs: &Vec<TxIn>) {
        self.tx_cache = TransactionCache::new(block_height, tx_index, tx_id);
        self.tx_cache.unallocate_input_rune_balance(tx_inputs);
    }

    /// Finalizes the current transaction index cache.
    pub fn end_transaction(&mut self) {
        self.tx_cache
            .allocate_remaining_balances(&mut self.db_cache);
    }

    pub async fn apply_etching(
        &mut self,
        etching: &Etching,
        _db_tx: &mut Transaction<'_>,
        _ctx: &Context,
    ) {
        let (rune_id, db_rune) =
            self.tx_cache
                .apply_etching(etching, self.next_rune_number, &mut self.db_cache);
        self.runes.put(rune_id, db_rune);
        self.next_rune_number += 1;
    }

    pub async fn apply_cenotaph_etching(
        &mut self,
        rune: &Rune,
        _db_tx: &mut Transaction<'_>,
        _ctx: &Context,
    ) {
        let (rune_id, db_rune) =
            self.tx_cache
                .apply_cenotaph_etching(rune, self.next_rune_number, &mut self.db_cache);
        self.runes.put(rune_id, db_rune);
        self.next_rune_number += 1;
    }

    pub async fn apply_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            // log
            return;
        };
        self.tx_cache
            .apply_mint(&rune_id, &db_rune, &mut self.db_cache);
    }

    pub async fn apply_cenotaph_mint(
        &mut self,
        rune_id: &RuneId,
        db_tx: &mut Transaction<'_>,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            // log
            return;
        };
        self.tx_cache
            .apply_mint(&rune_id, &db_rune, &mut self.db_cache);
    }

    pub async fn apply_edict(&mut self, edict: &Edict, db_tx: &mut Transaction<'_>, ctx: &Context) {
        let Some(db_rune) = self.get_rune_by_rune_id(&edict.id, db_tx, ctx).await else {
            // rune should exist?
            return;
        };
        self.tx_cache
            .apply_edict(edict, &db_rune, &mut self.db_cache);
    }

    async fn get_rune_by_rune_id(
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
        // TODO: Handle cache miss
        let Some(db_rune) = get_rune_by_rune_id(rune_id, db_tx, ctx).await else {
            return None;
        };
        self.runes.put(rune_id.clone(), db_rune.clone());
        return Some(db_rune);
    }
}
