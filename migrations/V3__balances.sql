CREATE TABLE IF NOT EXISTS balances (
    rune_id                 TEXT NOT NULL,
    address                 TEXT NOT NULL,
    balance                 NUMERIC NOT NULL,
    total_operations        BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (rune_id, address)
);

CREATE INDEX balances_address_balance_index ON balances (address, balance DESC);
CREATE INDEX balances_rune_id_balance_index ON balances (rune_id, balance DESC);
