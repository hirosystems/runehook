use ordinals::Etching;

#[macro_use]
extern crate hiro_system_kit;

#[macro_use]
extern crate serde_derive;

extern crate serde;

pub mod cli;
pub mod config;
pub mod db;
pub mod service;

fn main() {
    // cli::main();
    test_postgres();
}

fn test_postgres() -> Result<postgres::Client, postgres::Error> {
    use postgres::{Client, NoTls};

    let mut client = Client::connect("host=localhost user=postgres", NoTls)?;

    client.batch_execute(
        "
        CREATE TABLE etching (
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
    ",
    )?;

    Ok(client)
}

fn insert_etching(etching: &Etching, client: &postgres::Client) -> Result<bool, postgres::Error> {
    let Some(run_display) = etching.rune.map(|r| r.to_string()) else {
        println!("etching.rune = null");
        return Ok(false);
    };

    let rune_id = etching.rune.map(|r| r.0);
    let Some(divisibility) = etching.divisibility else {
        println!("divisibility = null");
        return Ok(false);
    };
    let Some(premine) = etching.premine else {
        println!("premine = null");
        return Ok(false);
    };
    let Some(terms_amount) = etching.terms.map(|t| t.amount.unwrap()) else {
        println!("terms_amount = null");
        return Ok(false);
    };
    let Some(terms_cap) = etching.terms.map(|t| t.cap.unwrap()) else {
        println!("terms_cap = null");
        return Ok(false);
    };
    let Some((terms_height_a, terms_height_b)) = etching
        .terms
        .map(|t| (t.height.0.unwrap(), t.height.1.unwrap()))
    else {
        println!("etching.terms = null");
        return Ok(false);
    };
    let Some((terms_offset_a, terms_offset_b)) = etching
        .terms
        .map(|t| (t.height.0.unwrap(), t.height.1.unwrap()))
    else {
        println!("etching.terms = null");
        return Ok(false);
    };

    client.execute(
        "INSERT INTO etching (divisibility, premine, rune, rune_display, spacers, symbol, amount, cap, height_a, height_b, offset_a, offset_b, turbo) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        &[&divisibility.to_string(), &premine, etching.rune.map(|r| r.0), &run_display, &etching.spacers, etching.symbol, terms_amount, terms_cap, terms_height_a, terms_height_b, terms_offset_a, terms_offset_b, etching.turbo],
    )?;

    // for row in client.query("SELECT id, name, data FROM person", &[])? {
    //     let id: i32 = row.get(0);
    //     let name: &str = row.get(1);
    //     let data: Option<&[u8]> = row.get(2);

    //     println!("found person: {} {} {:?}", id, name, data);
    // }

    Ok(())
}
