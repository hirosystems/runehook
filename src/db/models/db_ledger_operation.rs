/// A value from the `ledger_operation` enum type.
#[derive(Debug, Clone)]
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
