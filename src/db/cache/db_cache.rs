use chainhook_sdk::utils::Context;
use tokio_postgres::Transaction;

use crate::db::{
    insert_ledger_entries, insert_rune_rows,
    models::{db_ledger_entry::DbLedgerEntry, db_rune::DbRune},
};

/// Holds rows that have yet to be inserted into the database.
pub struct DbCache {
    pub runes: Vec<DbRune>,
    pub ledger_entries: Vec<DbLedgerEntry>,
}

impl DbCache {
    pub fn new() -> Self {
        DbCache {
            runes: Vec::new(),
            ledger_entries: Vec::new(),
        }
    }

    pub async fn flush(&mut self, db_tx: &mut Transaction<'_>, ctx: &Context) {
        if self.runes.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} rune rows",
                self.runes.len()
            );
            let _ = insert_rune_rows(&self.runes, db_tx, ctx).await;
            self.runes.clear();
        }
        if self.ledger_entries.len() > 0 {
            debug!(
                ctx.expect_logger(),
                "Flushing {} ledger rows",
                self.ledger_entries.len()
            );
            let _ = insert_ledger_entries(&self.ledger_entries, db_tx, ctx).await;
            self.ledger_entries.clear();
        }
    }
}
