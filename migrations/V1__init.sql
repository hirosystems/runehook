CREATE TABLE IF NOT EXISTS runes (
    number                  BIGINT NOT NULL PRIMARY KEY,
    name                    TEXT NOT NULL UNIQUE,
    block_height            NUMERIC NOT NULL,
    tx_index                BIGINT NOT NULL,
    tx_id                   TEXT NOT NULL,
    divisibility            SMALLINT,
    premine                 NUMERIC,
    symbol                  TEXT,
    terms_amount            NUMERIC,
    terms_cap               NUMERIC,
    terms_height_start      NUMERIC,
    terms_height_end        NUMERIC,
    terms_offset_start      NUMERIC,
    terms_offset_end        NUMERIC,
    turbo                   BOOLEAN NOT NULL,
    minted                  NUMERIC NOT NULL DEFAULT 0,
    burned                  NUMERIC NOT NULL DEFAULT 0
);
CREATE INDEX runes_block_height_tx_index_index ON runes (block_height, tx_index);

CREATE TYPE ledger_operation AS ENUM ('mint', 'burn', 'send', 'receive');

CREATE TABLE IF NOT EXISTS ledger (
    rune_number             BIGINT NOT NULL,
    block_height            NUMERIC NOT NULL,
    tx_index                BIGINT NOT NULL,
    tx_id                   TEXT NOT NULL,
    address                 TEXT NOT NULL,
    amount                  NUMERIC NOT NULL,
    operation               ledger_operation NOT NULL
);
CREATE INDEX ledger_rune_number_index ON ledger (rune_number);
CREATE INDEX ledger_block_height_tx_index_index ON ledger (block_height, tx_index);
CREATE INDEX ledger_address_rune_number_index ON ledger (address, rune_number);