use ordinals::RuneId;
use tokio_postgres::Row;

use crate::db::types::{
    pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64,
};

use super::db_ledger_operation::DbLedgerOperation;

/// A row in the `ledger` table.
#[derive(Debug, Clone)]
pub struct DbLedgerEntry {
    pub rune_id: String,
    pub block_height: PgNumericU64,
    pub tx_index: PgBigIntU32,
    pub event_index: PgBigIntU32,
    pub tx_id: String,
    pub output: Option<PgBigIntU32>,
    pub address: Option<String>,
    pub receiver_address: Option<String>,
    pub amount: PgNumericU128,
    pub operation: DbLedgerOperation,
    pub timestamp: PgBigIntU32,
}

impl DbLedgerEntry {
    pub fn from_values(
        amount: u128,
        rune_id: RuneId,
        block_height: u64,
        tx_index: u32,
        event_index: u32,
        tx_id: &String,
        output: Option<u32>,
        address: Option<&String>,
        receiver_address: Option<&String>,
        operation: DbLedgerOperation,
        timestamp: u32,
    ) -> Self {
        DbLedgerEntry {
            rune_id: rune_id.to_string(),
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            event_index: PgBigIntU32(event_index),
            tx_id: tx_id[2..].to_string(),
            output: output.map(|i| PgBigIntU32(i)),
            address: address.cloned(),
            receiver_address: receiver_address.cloned(),
            amount: PgNumericU128(amount),
            operation,
            timestamp: PgBigIntU32(timestamp),
        }
    }

    pub fn from_pg_row(row: &Row) -> Self {
        DbLedgerEntry {
            rune_id: row.get("rune_id"),
            block_height: row.get("block_height"),
            tx_index: row.get("tx_index"),
            event_index: row.get("event_index"),
            tx_id: row.get("tx_id"),
            output: row.get("output"),
            address: row.get("address"),
            receiver_address: row.get("receiver_address"),
            amount: row.get("amount"),
            operation: row.get("operation"),
            timestamp: row.get("timestamp"),
        }
    }
}
