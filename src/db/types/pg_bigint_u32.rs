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

#[cfg(test)]
mod test {
    use test_case::test_case;
    use tokio_postgres::NoTls;

    use super::PgBigIntU32;

    #[test_case(4294967295; "u32 max")]
    #[test_case(0; "zero")]
    #[tokio::test]
    async fn test_u32_to_postgres(val: u32) {
        let (mut client, connection) =
            tokio_postgres::connect("host=localhost user=postgres", NoTls)
                .await
                .unwrap();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let value = PgBigIntU32(val);
        let tx = client.transaction().await.unwrap();
        let _ = tx.query("CREATE TABLE test (value BIGINT)", &[]).await;
        let _ = tx
            .query("INSERT INTO test (value) VALUES ($1)", &[&value])
            .await;
        let row = tx.query_one("SELECT value FROM test", &[]).await.unwrap();
        let res: PgBigIntU32 = row.get("value");
        let _ = tx.rollback().await;
        assert_eq!(res.0, value.0);
    }
}
