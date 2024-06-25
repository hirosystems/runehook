CREATE TABLE IF NOT EXISTS balances (
    rune_id                 TEXT NOT NULL,
    address                 TEXT NOT NULL,
    balance                 NUMERIC NOT NULL,
    PRIMARY KEY (rune_id, address)
);

CREATE INDEX balances_address_balance_index ON (address, balance DESC);
CREATE INDEX balances_rune_id_balance_index ON (rune_id, balance DESC);
