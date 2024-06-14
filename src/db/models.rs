use ordinals::{Edict, Etching, Rune, SpacedRune};
use tokio_postgres::Row;

use super::types::{PgNumericU128, PgBigIntU32, PgNumericU64, PgSmallIntU8};

/// A value from the `ledger_operation` enum type.
#[derive(Debug)]
pub enum DbLedgerOperation {
    Mint,
    Burn,
    Send,
    Receive,
}

impl DbLedgerOperation {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Mint => "mint",
            Self::Burn => "burn",
            Self::Send => "send",
            Self::Receive => "receive",
        }
    }
}

impl std::str::FromStr for DbLedgerOperation {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mint" => Ok(DbLedgerOperation::Mint),
            "burn" => Ok(DbLedgerOperation::Burn),
            "send" => Ok(DbLedgerOperation::Send),
            "receive" => Ok(DbLedgerOperation::Receive),
            _ => Err(()),
        }
    }
}

/// A row in the `runes` table.
#[derive(Debug)]
pub struct DbRune {
    pub name: String,
    pub number: PgBigIntU32,
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
    pub burned: PgNumericU128,
}

impl DbRune {
    pub fn from_etching(
        etching: &Etching,
        number: u32,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
    ) -> Self {
        let name: String;
        if let Some(rune) = etching.rune {
            let spaced_rune = SpacedRune::new(rune, etching.spacers.unwrap_or(0));
            name = spaced_rune.to_string();
        } else {
            let rune = Rune::reserved(block_height, tx_index);
            name = rune.to_string();
        }
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
            name,
            number: PgBigIntU32(number),
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            tx_id: tx_id[2..].to_string(),
            divisibility: etching.divisibility.map(|i| PgSmallIntU8(i)).unwrap_or(PgSmallIntU8(0)),
            premine: etching.premine.map(|i| PgNumericU128(i)).unwrap_or(PgNumericU128(0)),
            symbol: etching.symbol.map(|i| i.to_string()).unwrap_or("¤".to_string()),
            terms_amount,
            terms_cap,
            terms_height_start,
            terms_height_end,
            terms_offset_start,
            terms_offset_end,
            turbo: etching.turbo,
            minted: PgNumericU128(0),
            burned: PgNumericU128(0),
        }
    }

    pub fn from_pg_row(row: &Row) -> Self {
        DbRune {
            name: row.get("name"),
            number: row.get("number"),
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
            burned: row.get("burned"),
        }
    }
}

/// A row in the `ledger` table.
#[derive(Debug)]
pub struct DbLedgerEntry {
    pub rune_number: PgBigIntU32,
    pub block_height: PgNumericU64,
    pub tx_index: PgBigIntU32,
    pub tx_id: String,
    pub address: String,
    pub amount: PgNumericU128,
    pub operation: DbLedgerOperation,
}

impl DbLedgerEntry {
    pub fn from_edict(
        edict: &Edict,
        db_rune: &DbRune,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        address: &String,
        operation: DbLedgerOperation,
    ) -> Self {
        DbLedgerEntry {
            rune_number: db_rune.number,
            block_height: PgNumericU64(block_height),
            tx_index: PgBigIntU32(tx_index),
            tx_id: tx_id[2..].to_string(),
            address: address.clone(),
            amount: PgNumericU128(edict.amount),
            operation,
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
            1,
            0,
            &"14e87956a6bb0f50df1515e85f1dcc4625a7e2ebeb08ab6db7d9211c7cf64fa3".to_string(),
        );
        assert!(db_rune.name == "UNCOMMON•GOODS");
    }
}
