use std::error::Error;

use bytes::{BufMut, BytesMut};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

#[derive(Debug, Clone, Copy)]
pub struct PgSmallIntU8(pub u8);

impl ToSql for PgSmallIntU8 {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.put_u16(self.0 as u16);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "int2" || ty.name() == "smallint"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgSmallIntU8 {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<PgSmallIntU8, Box<dyn Error + Sync + Send>> {
        let mut arr = [0u8; 1];
        arr.copy_from_slice(&raw[1..2]);
        Ok(PgSmallIntU8(u8::from_be_bytes(arr)))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "int2" || ty.name() == "smallint"
    }
}