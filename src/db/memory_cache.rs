use chainhook_sdk::utils::Context;
use ordinals::{Edict, Etching, RuneId};
use postgres::Transaction;

use super::{
    get_rune_by_rune_id, insert_rune_rows,
    model::{DbLedgerEntry, DbLedgerOperation, DbRune},
};

pub struct DbCache {
    pub runes: Vec<DbRune>,
    pub ledger_entries: Vec<DbLedgerEntry>,
}

impl DbCache {
    fn new() -> Self {
        DbCache {
            runes: vec![],
            ledger_entries: vec![],
        }
    }

    pub fn flush(&mut self, db_tx: &mut Transaction, ctx: &Context) {
        if self.runes.len() > 0 {
            let _ = insert_rune_rows(&self.runes, db_tx, ctx);
        }
        if self.ledger_entries.len() > 0 {
            //
        }
    }
}

pub struct MemoryCache {
    pub max_rune_number: u64,
    pub db_cache: DbCache,
}

impl MemoryCache {
    pub fn new() -> Self {
        MemoryCache {
            db_cache: DbCache::new(),
            // TODO: get from db
            max_rune_number: 0,
        }
    }

    pub fn insert_etching(
        &mut self,
        etching: &Etching,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        _db_tx: &Transaction,
        _ctx: &Context,
    ) {
        self.max_rune_number += 1;
        self.db_cache.runes.push(DbRune::from_etching(
            etching,
            self.max_rune_number,
            block_height,
            tx_index,
            tx_id,
        ));
    }

    pub fn insert_edict(
        &mut self,
        edict: &Edict,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        address: &String,
        db_tx: &mut Transaction,
        ctx: &Context,
    ) {
        let Some(db_rune) = self.get_rune_by_rune_id(edict.id, db_tx, ctx) else {
            // log
            return;
        };
        self.db_cache.ledger_entries.push(DbLedgerEntry::from_edict(
            edict,
            db_rune,
            block_height,
            tx_index,
            tx_id,
            address,
            DbLedgerOperation::Send,
        ));
    }

    fn get_rune_by_rune_id(&mut self, rune_id: RuneId, db_tx: &mut Transaction, ctx: &Context) -> Option<DbRune> {
        get_rune_by_rune_id(rune_id, db_tx, ctx)
    }
}
