use std::error::Error;

use bytes::{BufMut, BytesMut};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

#[derive(Debug, Clone, Copy)]
pub struct PgBigIntU32(pub u32);

impl ToSql for PgBigIntU32 {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.put_u64(self.0 as u64);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "int8" || ty.name() == "bigint"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgBigIntU32 {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<PgBigIntU32, Box<dyn Error + Sync + Send>> {
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&raw[4..8]);
        Ok(PgBigIntU32(u32::from_be_bytes(arr)))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "int8" || ty.name() == "bigint"
    }
}
