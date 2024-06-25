use crate::db::types::pg_numeric_u128::PgNumericU128;

#[derive(Debug, Clone)]
pub struct DbBalanceUpdate {
    pub rune_id: String,
    pub address: String,
    pub balance: PgNumericU128,
}
