use std::error::Error;

use bytes::BytesMut;
use num_traits::ToPrimitive;
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

use super::pg_numeric_u128::{pg_numeric_bytes_to_u128, u128_into_pg_numeric_bytes};

#[derive(Debug, Clone, Copy)]
pub struct PgNumericU64(pub u64);

impl ToSql for PgNumericU64 {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        u128_into_pg_numeric_bytes(self.0 as u128, out);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgNumericU64 {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<PgNumericU64, Box<dyn Error + Sync + Send>> {
        let result = pg_numeric_bytes_to_u128(raw);
        Ok(PgNumericU64(result.to_u64().unwrap()))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }
}
