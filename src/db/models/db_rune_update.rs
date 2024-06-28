use crate::db::types::{pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128};

/// An update to a rune that affects its total counts.
#[derive(Debug, Clone)]
pub struct DbRuneUpdate {
    pub id: String,
    pub minted: PgNumericU128,
    pub total_mints: PgBigIntU32,
    pub burned: PgNumericU128,
    pub total_burns: PgBigIntU32,
    pub total_operations: PgBigIntU32,
}

impl DbRuneUpdate {
    pub fn from_mint(id: String, amount: PgNumericU128) -> Self {
        DbRuneUpdate {
            id,
            minted: amount,
            total_mints: PgBigIntU32(1),
            burned: PgNumericU128(0),
            total_burns: PgBigIntU32(0),
            total_operations: PgBigIntU32(1),
        }
    }

    pub fn from_burn(id: String, amount: PgNumericU128) -> Self {
        DbRuneUpdate {
            id,
            minted: PgNumericU128(0),
            total_mints: PgBigIntU32(0),
            burned: amount,
            total_burns: PgBigIntU32(1),
            total_operations: PgBigIntU32(1),
        }
    }

    pub fn from_operation(id: String) -> Self {
        DbRuneUpdate {
            id,
            minted: PgNumericU128(0),
            total_mints: PgBigIntU32(0),
            burned: PgNumericU128(0),
            total_burns: PgBigIntU32(0),
            total_operations: PgBigIntU32(1),
        }
    }
}
