use chainhook_sdk::utils::Context;
use model::DbRune;
use ordinals::RuneId;
use postgres::{Client, Error, NoTls, Transaction};

pub mod memory_cache;
pub mod model;

pub fn init_db(ctx: &Context) -> Result<Client, Error> {
    let mut client = Client::connect("host=localhost user=postgres", NoTls)?;
    // TODO: use migrations with `refinery`
    client.batch_execute(
        "
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
            terms_height_start      BIGINT,
            terms_height_end        BIGINT,
            terms_offset_start      BIGINT,
            terms_offset_end        BIGINT,
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
    ",
    )?;
    // Insert default UNCOMMON•GOODS rune
    // let rune = SpacedRune::from_str("UNCOMMON•GOODS").unwrap();
    // let _ = insert_etching(
    //     &Etching {
    //         divisibility: Some(0),
    //         premine: Some(0),
    //         rune: Some(rune.rune),
    //         spacers: Some(rune.spacers),
    //         symbol: Some('⧉'),
    //         terms: Some(Terms {
    //             amount: Some(1),
    //             cap: Some(u128::max_value()),
    //             height: (Some(840000), Some(1050000)),
    //             offset: (None, None),
    //         }),
    //         turbo: false,
    //     },
    //     1,
    //     0,
    //     &"".to_string(),
    //     &mut client,
    //     ctx,
    // );
    Ok(client)
}

pub fn insert_rune_rows(
    rows: &Vec<DbRune>,
    db_tx: &mut Transaction,
    _ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx.prepare(
        "INSERT INTO runes
        (name, block_height, tx_index, tx_id, divisibility, premine, symbol, terms_amount, terms_cap, terms_height_start,
         terms_height_end, terms_offset_start, terms_offset_end, turbo)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (name) DO NOTHING"
    )?;
    for row in rows.iter() {
        let _ = db_tx.execute(
            &stmt,
            &[
                &row.name,
                &row.block_height,
                &row.tx_index,
                &row.tx_id,
                &row.divisibility,
                &row.premine,
                &row.symbol,
                &row.terms_amount,
                &row.terms_cap,
                &row.terms_height_start,
                &row.terms_height_end,
                &row.terms_offset_start,
                &row.terms_offset_end,
                &row.turbo,
            ],
        );
    }
    Ok(true)
}

// pub fn insert_edict(
//     edict: &Edict,
//     block_height: u64,
//     tx_index: u32,
//     tx_id: &String,
//     client: &mut Client,
//     ctx: &Context,
// ) -> Result<bool, Error> {
//     //
// }

pub fn get_rune_by_rune_id(rune_id: RuneId, db_tx: &mut Transaction, ctx: &Context) -> Option<DbRune> {
    let rows = match db_tx.query(
        "SELECT * FROM runes WHERE block_height = $1 AND tx_index = $2",
        &[&rune_id.block.to_string(), &rune_id.tx.to_string()],
    ) {
        Ok(rows) => rows,
        Err(e) => {
            error!(
                ctx.expect_logger(),
                "error retrieving rune: {}",
                e.to_string()
            );
            panic!();
        }
    };
    let Some(row) = rows.get(0) else {
        return None;
    };
    Some(DbRune::from_pg_row(row))
}
