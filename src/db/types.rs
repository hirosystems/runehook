use bytes::{BufMut, BytesMut};
use rust_decimal::{prelude::FromPrimitive, Decimal};
use std::error::Error;
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

use super::models::DbLedgerOperation;

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

#[derive(Debug, Clone, Copy)]
pub struct PgNumericU64(pub u64);

impl ToSql for PgNumericU64 {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        Decimal::from_u64(self.0).to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgNumericU64 {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<PgNumericU64, Box<dyn Error + Sync + Send>> {
        Ok(PgNumericU64(Decimal::from_sql(ty, raw)?.to_string().parse::<u64>()?))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PgNumericU128(pub u128);

impl ToSql for PgNumericU128 {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        Decimal::from_u128(self.0).to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for PgNumericU128 {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<PgNumericU128, Box<dyn Error + Sync + Send>> {
        Ok(PgNumericU128(Decimal::from_sql(ty, raw)?.to_string().parse::<u128>()?))
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "numeric"
    }
}

impl ToSql for DbLedgerOperation {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(self.as_str().as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "text"
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for DbLedgerOperation {
    fn from_sql(
        _ty: &Type,
        raw: &'a [u8],
    ) -> Result<DbLedgerOperation, Box<dyn Error + Sync + Send>> {
        let s = std::str::from_utf8(raw)?;
        s.parse::<DbLedgerOperation>()
            .map_err(|_| "failed to parse enum variant".into())
    }

    fn accepts(ty: &Type) -> bool {
        ty.name() == "text"
    }
}
