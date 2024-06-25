CREATE TABLE IF NOT EXISTS runes (
    id                      TEXT NOT NULL PRIMARY KEY,
    number                  BIGINT NOT NULL UNIQUE,
    name                    TEXT NOT NULL UNIQUE,
    spaced_name             TEXT NOT NULL,
    block_height            NUMERIC NOT NULL,
    tx_index                BIGINT NOT NULL,
    tx_id                   TEXT NOT NULL,
    divisibility            SMALLINT NOT NULL DEFAULT 0,
    premine                 NUMERIC NOT NULL DEFAULT 0,
    symbol                  TEXT NOT NULL DEFAULT '¤',
    terms_amount            NUMERIC,
    terms_cap               NUMERIC,
    terms_height_start      NUMERIC,
    terms_height_end        NUMERIC,
    terms_offset_start      NUMERIC,
    terms_offset_end        NUMERIC,
    turbo                   BOOLEAN NOT NULL DEFAULT FALSE,
    minted                  NUMERIC NOT NULL DEFAULT 0,
    total_mints             BIGINT NOT NULL DEFAULT 0,
    burned                  NUMERIC NOT NULL DEFAULT 0,
    total_burns             BIGINT NOT NULL DEFAULT 0,
    total_operations        BIGINT NOT NULL DEFAULT 0,
    timestamp               BIGINT NOT NULL
);
CREATE INDEX runes_block_height_tx_index_index ON runes (block_height, tx_index);

-- Insert default 'UNCOMMON•GOODS'
INSERT INTO runes (
    id, number, name, spaced_name, block_height, tx_index, tx_id, symbol, terms_amount, terms_cap,
    terms_height_start, terms_height_end, timestamp
)
VALUES (
    '1:0', 0, 'UNCOMMONGOODS', 'UNCOMMON•GOODS', 840000, 0, '', '⧉', 1, '340282366920938463463374607431768211455', 840000,
    1050000, 0
);
