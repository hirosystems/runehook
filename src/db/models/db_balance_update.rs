use crate::db::types::{pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128};

#[derive(Debug, Clone)]
pub struct DbBalanceUpdate {
    pub rune_id: String,
    pub address: String,
    pub balance: PgNumericU128,
    pub total_operations: PgBigIntU32,
}

impl DbBalanceUpdate {
    pub fn from_operation(rune_id: String, address: String, balance: PgNumericU128) -> Self {
        DbBalanceUpdate { rune_id, address, balance, total_operations: PgBigIntU32(1) }
    }
}
