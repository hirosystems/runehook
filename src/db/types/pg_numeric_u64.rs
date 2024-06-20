use std::error::Error;

use bytes::BytesMut;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

use super::pg_numeric_u128::{big_uint_from_pg_numeric_bytes, write_big_uint_to_pg_numeric_bytes};

#[derive(Debug, Clone, Copy)]
pub struct PgNumericU64(pub u64);

impl ToSql for PgNumericU64 {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let num =
            BigInt::parse_bytes(&self.0.to_string().as_bytes(), 10).expect("Invalid number string");
        write_big_uint_to_pg_numeric_bytes(num, out);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgNumericU64 {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<PgNumericU64, Box<dyn Error + Sync + Send>> {
        let result = big_uint_from_pg_numeric_bytes(raw);
        Ok(PgNumericU64(result.to_u64().unwrap()))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }
}
