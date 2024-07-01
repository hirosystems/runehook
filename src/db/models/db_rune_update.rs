use crate::db::types::pg_numeric_u128::PgNumericU128;

/// An update to a rune that affects its total counts.
#[derive(Debug, Clone)]
pub struct DbRuneUpdate {
    pub id: String,
    pub minted: PgNumericU128,
    pub total_mints: PgNumericU128,
    pub burned: PgNumericU128,
    pub total_burns: PgNumericU128,
    pub total_operations: PgNumericU128,
}

impl DbRuneUpdate {
    pub fn from_mint(id: String, amount: PgNumericU128) -> Self {
        DbRuneUpdate {
            id,
            minted: amount,
            total_mints: PgNumericU128(1),
            burned: PgNumericU128(0),
            total_burns: PgNumericU128(0),
            total_operations: PgNumericU128(1),
        }
    }

    pub fn from_burn(id: String, amount: PgNumericU128) -> Self {
        DbRuneUpdate {
            id,
            minted: PgNumericU128(0),
            total_mints: PgNumericU128(0),
            burned: amount,
            total_burns: PgNumericU128(1),
            total_operations: PgNumericU128(1),
        }
    }

    pub fn from_operation(id: String) -> Self {
        DbRuneUpdate {
            id,
            minted: PgNumericU128(0),
            total_mints: PgNumericU128(0),
            burned: PgNumericU128(0),
            total_burns: PgNumericU128(0),
            total_operations: PgNumericU128(1),
        }
    }
}
