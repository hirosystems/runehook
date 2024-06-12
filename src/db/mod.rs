use chainhook_sdk::utils::Context;
use postgres::{Client, NoTls, Error};
use ordinals::Etching;

pub fn init_db() -> Result<Client, Error> {
    let mut client = Client::connect("host=localhost user=postgres", NoTls)?;
    client.batch_execute("
        CREATE TABLE IF NOT EXISTS etching (
            rune                    TEXT NOT NULL PRIMARY KEY,
            divisibility            SMALLINT,
            premine                 NUMERIC,
            spacers                 BIGINT,
            symbol                  TEXT,
            terms_amount            NUMERIC,
            terms_cap               NUMERIC,
            terms_height_start      BIGINT,
            terms_height_end        BIGINT,
            terms_offset_start      BIGINT,
            terms_offset_end        BIGINT,
            turbo                   BOOLEAN NOT NULL
        )
    ")?;
    Ok(client)
}

pub fn insert_etching(etching: &Etching, client: &mut Client, ctx: &Context) -> Result<bool, Error> {
    let Some(rune_display) = etching.rune.map(|r| r.to_string()) else {
        println!("etching.rune = null");
        return Ok(false);
    };
    let divisibility = etching.divisibility.map(|r| r.to_string());
    let premine = etching.premine.map(|p| p.to_string());
    let spacers = etching.spacers.map(|i| i.to_string());
    let symbol = etching.symbol.map(|i| i.to_string());
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

    let res = client.execute(
        "INSERT INTO etching
        (rune, divisibility, premine, spacers, symbol, terms_amount, terms_cap, terms_height_start, terms_height_end, terms_offset_start, terms_offset_end, turbo)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        &[&rune_display, &divisibility, &premine, &spacers, &symbol, &terms_amount, &terms_cap, &terms_height_start, &terms_height_end, &terms_offset_start, &terms_offset_end, &etching.turbo],
    );

    if let Err(e) = res {
        error!(
            ctx.expect_logger(),
            "Error inserting: {}", e.to_string(),
        );

    }
    Ok(true)
}
