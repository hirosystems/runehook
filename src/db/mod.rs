use bytes::BytesMut;
use postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};
use std::error::Error;

// impl ToSql for u8 {
//     fn to_sql(
//         &self,
//         _ty: &Type,
//         out: &mut BytesMut,
//         _context: &mut (),
//     ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
//         out.put_i16(*self as i16);
//         Ok(IsNull::No)
//     }

//     fn accepts(ty: &Type) -> bool {
//         ty.name() == "smallint"
//     }

//     to_sql_checked!();
// }

// impl<'a> FromSql<'a> for u8 {
//     fn from_sql(
//         _ty: &Type,
//         raw: &'a [u8],
//     ) -> Result<u8, Box<dyn Error + Sync + Send>> {
//         if raw.len() != 2 {
//             return Err("Invalid length for u8".into());
//         }
//         let mut arr = [0u8; 2];
//         arr.copy_from_slice(raw);
//         let value = i16::from_be_bytes(arr);
//         if value < 0 || value > 255 {
//             return Err("Value out of range for u8".into());
//         }
//         Ok(value as u8)
//     }

//     fn accepts(ty: &Type) -> bool {
//         ty.name() == "smallint"
//     }
// }