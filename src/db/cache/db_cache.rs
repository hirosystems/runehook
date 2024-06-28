use std::collections::HashMap;

use chainhook_sdk::utils::Context;
use tokio_postgres::Transaction;

use crate::db::{
    models::{
        db_balance_update::DbBalanceUpdate, db_ledger_entry::DbLedgerEntry, db_rune::DbRune,
        db_rune_update::DbRuneUpdate,
    },
    pg_insert_ledger_entries, pg_insert_rune_rows, pg_update_balances, pg_update_runes,
};

/// Holds rows that have yet to be inserted into the database.
pub struct DbCache {
    pub runes: Vec<DbRune>,
    pub ledger_entries: Vec<DbLedgerEntry>,
    pub rune_updates: HashMap<String, DbRuneUpdate>,
    pub balance_increases: HashMap<(String, String), DbBalanceUpdate>,
    pub balance_deductions: HashMap<(String, String), DbBalanceUpdate>,
}

impl DbCache {
    pub fn new() -> Self {
        DbCache {
            runes: Vec::new(),
            ledger_entries: Vec::new(),
            rune_updates: HashMap::new(),
            balance_increases: HashMap::new(),
            balance_deductions: HashMap::new(),
        }
    }

    /// Insert all data into the DB and clear cache.
    pub async fn flush(&mut self, db_tx: &mut Transaction<'_>, ctx: &Context) {
        if self.runes.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} rune rows",
                self.runes.len()
            );
            let _ = pg_insert_rune_rows(&self.runes, db_tx, ctx).await;
            self.runes.clear();
        }
        if self.rune_updates.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} rune updates",
                self.rune_updates.len()
            );
            let _ =
                pg_update_runes(&self.rune_updates.values().cloned().collect(), db_tx, ctx).await;
            self.rune_updates.clear();
        }
        if self.ledger_entries.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} ledger rows",
                self.ledger_entries.len()
            );
            let _ = pg_insert_ledger_entries(&self.ledger_entries, db_tx, ctx).await;
            self.ledger_entries.clear();
        }
        if self.balance_increases.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} balance increases",
                self.balance_increases.len()
            );
            let _ = pg_update_balances(
                &self.balance_increases.values().cloned().collect(),
                true,
                db_tx,
                ctx,
            )
            .await;
            self.balance_increases.clear();
        }
        if self.balance_deductions.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} balance deductions",
                self.balance_deductions.len()
            );
            let _ = pg_update_balances(
                &self.balance_deductions.values().cloned().collect(),
                false,
                db_tx,
                ctx,
            )
            .await;
            self.balance_deductions.clear();
        }
    }
}
