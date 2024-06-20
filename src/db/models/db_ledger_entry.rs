use ordinals::RuneId;

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
    pub tx_id: String,
    pub output: PgBigIntU32,
    pub address: String,
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
        tx_id: &String,
        output: u32,
        address: &String,
        operation: DbLedgerOperation,
        timestamp: u32,
    ) -> Self {
        DbLedgerEntry {
            rune_id: rune_id.to_string(),
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            tx_id: tx_id[2..].to_string(),
            output: PgBigIntU32(output),
            address: address.clone(),
            amount: PgNumericU128(amount),
            operation,
            timestamp: PgBigIntU32(timestamp),
        }
    }
}
