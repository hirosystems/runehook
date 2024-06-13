use ordinals::{Edict, Etching, Rune, SpacedRune};
use tokio_postgres::Row;

pub enum DbLedgerOperation {
    Mint,
    Burn,
    Send,
    Receive,
}

impl DbLedgerOperation {
    pub fn to_string(&self) -> String {
        match self {
            Self::Mint => "mint".to_string(),
            Self::Burn => "burn".to_string(),
            Self::Send => "send".to_string(),
            Self::Receive => "receive".to_string(),
        }
    }
}

pub struct DbRune {
    pub name: String,
    pub number: String,
    pub block_height: String,
    pub tx_index: String,
    pub tx_id: String,
    pub divisibility: Option<String>,
    pub premine: Option<String>,
    pub symbol: Option<String>,
    pub terms_amount: Option<String>,
    pub terms_cap: Option<String>,
    pub terms_height_start: Option<String>,
    pub terms_height_end: Option<String>,
    pub terms_offset_start: Option<String>,
    pub terms_offset_end: Option<String>,
    pub turbo: bool,
    pub minted: String,
    pub burned: String,
}

impl DbRune {
    pub fn from_etching(
        etching: &Etching,
        number: u64,
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
            terms_amount = terms.amount.map(|i| i.to_string());
            terms_cap = terms.cap.map(|i| i.to_string());
            terms_height_start = terms.height.0.map(|i| i.to_string());
            terms_height_end = terms.height.1.map(|i| i.to_string());
            terms_offset_start = terms.offset.0.map(|i| i.to_string());
            terms_offset_end = terms.offset.1.map(|i| i.to_string());
        }
        DbRune {
            name,
            number: number.to_string(),
            block_height: block_height.to_string(),
            tx_index: tx_index.to_string(),
            tx_id: tx_id.clone(),
            divisibility: etching.divisibility.map(|r| r.to_string()),
            premine: etching.premine.map(|p| p.to_string()),
            symbol: etching.symbol.map(|i| i.to_string()),
            terms_amount,
            terms_cap,
            terms_height_start,
            terms_height_end,
            terms_offset_start,
            terms_offset_end,
            turbo: etching.turbo,
            minted: "0".to_string(),
            burned: "0".to_string(),
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

pub struct DbLedgerEntry {
    pub rune_number: String,
    pub block_height: String,
    pub tx_index: String,
    pub tx_id: String,
    pub address: String,
    pub amount: String,
    pub operation: String,
}

impl DbLedgerEntry {
    pub fn from_edict(
        edict: &Edict,
        db_rune: DbRune,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        address: &String,
        operation: DbLedgerOperation,
    ) -> Self {
        DbLedgerEntry {
            rune_number: db_rune.number,
            block_height: block_height.to_string(),
            tx_index: tx_index.to_string(),
            tx_id: tx_id.clone(),
            address: address.clone(),
            amount: edict.amount.to_string(),
            operation: operation.to_string(),
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
