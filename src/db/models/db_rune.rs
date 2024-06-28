use ordinals::{Etching, Rune, RuneId, SpacedRune};
use tokio_postgres::Row;

use crate::db::types::{
    pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64,
    pg_smallint_u8::PgSmallIntU8,
};

/// A row in the `runes` table.
#[derive(Debug, Clone)]
pub struct DbRune {
    pub id: String,
    pub number: PgBigIntU32,
    pub name: String,
    pub spaced_name: String,
    pub block_hash: String,
    pub block_height: PgNumericU64,
    pub tx_index: PgBigIntU32,
    pub tx_id: String,
    pub divisibility: PgSmallIntU8,
    pub premine: PgNumericU128,
    pub symbol: String,
    pub terms_amount: Option<PgNumericU128>,
    pub terms_cap: Option<PgNumericU128>,
    pub terms_height_start: Option<PgNumericU64>,
    pub terms_height_end: Option<PgNumericU64>,
    pub terms_offset_start: Option<PgNumericU64>,
    pub terms_offset_end: Option<PgNumericU64>,
    pub turbo: bool,
    pub minted: PgNumericU128,
    pub total_mints: PgBigIntU32,
    pub burned: PgNumericU128,
    pub total_burns: PgBigIntU32,
    pub total_operations: PgBigIntU32,
    pub timestamp: PgBigIntU32,
}

impl DbRune {
    pub fn from_etching(
        etching: &Etching,
        number: u32,
        block_hash: &String,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) -> Self {
        let rune = etching
            .rune
            .unwrap_or(Rune::reserved(block_height, tx_index));
        let spaced_name = if let Some(spacers) = etching.spacers {
            let spaced_rune = SpacedRune::new(rune, spacers);
            spaced_rune.to_string()
        } else {
            rune.to_string()
        };
        let name = rune.to_string();
        let mut terms_amount = None;
        let mut terms_cap = None;
        let mut terms_height_start = None;
        let mut terms_height_end = None;
        let mut terms_offset_start = None;
        let mut terms_offset_end = None;
        if let Some(terms) = etching.terms {
            terms_amount = terms.amount.map(|i| PgNumericU128(i));
            terms_cap = terms.cap.map(|i| PgNumericU128(i));
            terms_height_start = terms.height.0.map(|i| PgNumericU64(i));
            terms_height_end = terms.height.1.map(|i| PgNumericU64(i));
            terms_offset_start = terms.offset.0.map(|i| PgNumericU64(i));
            terms_offset_end = terms.offset.1.map(|i| PgNumericU64(i));
        }
        DbRune {
            id: format!("{}:{}", block_height, tx_index),
            number: PgBigIntU32(number),
            name,
            spaced_name,
            block_hash: block_hash[2..].to_string(),
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            tx_id: tx_id[2..].to_string(),
            divisibility: etching
                .divisibility
                .map(|i| PgSmallIntU8(i))
                .unwrap_or(PgSmallIntU8(0)),
            premine: etching
                .premine
                .map(|i| PgNumericU128(i))
                .unwrap_or(PgNumericU128(0)),
            symbol: etching
                .symbol
                .map(|i| i.to_string().replace('\0', ""))
                .unwrap_or("¤".to_string()),
            terms_amount,
            terms_cap,
            terms_height_start,
            terms_height_end,
            terms_offset_start,
            terms_offset_end,
            turbo: etching.turbo,
            minted: PgNumericU128(0),
            total_mints: PgBigIntU32(0),
            burned: PgNumericU128(0),
            total_burns: PgBigIntU32(0),
            total_operations: PgBigIntU32(0),
            timestamp: PgBigIntU32(timestamp),
        }
    }

    pub fn from_cenotaph_etching(
        rune: &Rune,
        number: u32,
        block_hash: &String,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) -> Self {
        DbRune {
            id: format!("{}:{}", block_height, tx_index),
            name: rune.to_string(),
            spaced_name: rune.to_string(),
            number: PgBigIntU32(number),
            block_hash: block_hash[2..].to_string(),
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            tx_id: tx_id[2..].to_string(),
            divisibility: PgSmallIntU8(0),
            premine: PgNumericU128(0),
            symbol: "".to_string(),
            terms_amount: None,
            terms_cap: None,
            terms_height_start: None,
            terms_height_end: None,
            terms_offset_start: None,
            terms_offset_end: None,
            turbo: false,
            minted: PgNumericU128(0),
            total_mints: PgBigIntU32(0),
            burned: PgNumericU128(0),
            total_burns: PgBigIntU32(0),
            total_operations: PgBigIntU32(0),
            timestamp: PgBigIntU32(timestamp),
        }
    }

    pub fn from_pg_row(row: &Row) -> Self {
        DbRune {
            id: row.get("id"),
            number: row.get("number"),
            name: row.get("name"),
            spaced_name: row.get("spaced_name"),
            block_hash: row.get("block_hash"),
            block_height: row.get("block_height"),
            tx_index: row.get("tx_index"),
            tx_id: row.get("tx_id"),
            divisibility: row.get("divisibility"),
            premine: row.get("premine"),
            symbol: row.get("symbol"),
            terms_amount: row.get("terms_amount"),
            terms_cap: row.get("terms_cap"),
            terms_height_start: row.get("terms_height_start"),
            terms_height_end: row.get("terms_height_end"),
            terms_offset_start: row.get("terms_offset_start"),
            terms_offset_end: row.get("terms_offset_end"),
            turbo: row.get("turbo"),
            minted: row.get("minted"),
            total_mints: row.get("total_mints"),
            burned: row.get("burned"),
            total_burns: row.get("total_burns"),
            total_operations: row.get("total_operations"),
            timestamp: row.get("timestamp"),
        }
    }

    pub fn rune_id(&self) -> RuneId {
        RuneId {
            block: self.block_height.0,
            tx: self.tx_index.0,
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use ordinals::{Etching, SpacedRune, Terms};

    use super::DbRune;

    #[test]
    fn test_from_etching() {
        let rune = SpacedRune::from_str("UNCOMMON•GOODS").unwrap();
        let db_rune = DbRune::from_etching(
            &Etching {
                divisibility: Some(0),
                premine: Some(0),
                rune: Some(rune.rune),
                spacers: Some(rune.spacers),
                symbol: Some('⧉'),
                terms: Some(Terms {
                    amount: Some(1),
                    cap: Some(u128::max_value()),
                    height: (Some(840000), Some(1050000)),
                    offset: (None, None),
                }),
                turbo: false,
            },
            0,
            &"00000000000000000000d2845e9e48d356e89fd3b2e1f3da668ffc04c7dfe298".to_string(),
            1,
            0,
            &"14e87956a6bb0f50df1515e85f1dcc4625a7e2ebeb08ab6db7d9211c7cf64fa3".to_string(),
            0,
        );
        assert!(db_rune.name == "UNCOMMONGOODS");
        assert!(db_rune.spaced_name == "UNCOMMON•GOODS");
    }
}
