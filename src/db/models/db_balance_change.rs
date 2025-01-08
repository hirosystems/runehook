use crate::db::types::{
    pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64,
};

#[derive(Debug, Clone, Default)]
pub struct DbBalanceChange {
    pub rune_id: String,
    pub block_height: PgNumericU64,
    pub address: String,
    pub balance: PgNumericU128,
    pub total_operations: PgBigIntU32,
}

impl DbBalanceChange {
    pub fn from_operation(
        rune_id: String,
        block_height: PgNumericU64,
        address: String,
        balance: PgNumericU128,
    ) -> Self {
        DbBalanceChange {
            rune_id,
            block_height,
            address,
            balance,
            total_operations: PgBigIntU32(1),
        }
    }
}
