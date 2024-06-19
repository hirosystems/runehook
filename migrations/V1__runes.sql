CREATE TABLE IF NOT EXISTS runes (
    number                  BIGINT NOT NULL PRIMARY KEY,
    name                    TEXT NOT NULL UNIQUE,
    spaced_name             TEXT NOT NULL,
    block_height            NUMERIC NOT NULL,
    tx_index                BIGINT NOT NULL,
    tx_id                   TEXT NOT NULL,
    timestamp               BIGINT NOT NULL,
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
);
CREATE INDEX runes_block_height_tx_index_index ON runes (block_height, tx_index);

-- Insert default 'UNCOMMON•GOODS'
INSERT INTO runes (
    number, name, block_height, tx_index, tx_id, symbol, terms_amount, terms_cap,
    terms_height_start, terms_height_end
)
VALUES (
    0, 'UNCOMMON•GOODS', 1, 0, '', '⧉', 1, '340282366920938463463374607431768211455', 840000,
    1050000
);
