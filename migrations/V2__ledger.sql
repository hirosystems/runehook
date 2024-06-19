CREATE TYPE ledger_operation AS ENUM ('mint', 'burn', 'send', 'receive');

CREATE TABLE IF NOT EXISTS ledger (
    rune_id                 TEXT NOT NULL,
    block_height            NUMERIC NOT NULL,
    tx_index                BIGINT NOT NULL,
    tx_id                   TEXT NOT NULL,
    output                  BIGINT NOT NULL,
    address                 TEXT NOT NULL,
    amount                  NUMERIC NOT NULL,
    operation               ledger_operation NOT NULL,
    timestamp               BIGINT NOT NULL
);

CREATE INDEX ledger_rune_id_index ON ledger (rune_id);
CREATE INDEX ledger_block_height_tx_index_index ON ledger (block_height, tx_index);
CREATE INDEX ledger_address_rune_id_index ON ledger (address, rune_id);
CREATE INDEX ledger_tx_id_output_index ON ledger (tx_id, output);
