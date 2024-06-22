use std::{collections::HashMap, str::FromStr};

use chainhook_sdk::utils::Context;
use models::{db_ledger_entry::DbLedgerEntry, db_rune::DbRune};
use ordinals::RuneId;
use refinery::embed_migrations;
use tokio_postgres::{types::ToSql, Client, Error, NoTls, Transaction};
use types::{
    pg_bigint_u32::PgBigIntU32, pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64,
};

use crate::config::Config;

pub mod cache;
pub mod index;
pub mod models;
pub mod types;

embed_migrations!("migrations");

pub async fn pg_connect(_config: &Config, run_migrations: bool, ctx: &Context) -> Client {
    let mut pg_client: Client;
    loop {
        match tokio_postgres::connect("host=localhost user=postgres", NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("connection error: {}", e);
                    }
                });
                pg_client = client;
                break;
            }
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error connecting to postgres: {}",
                    e.to_string()
                );
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    if run_migrations {
        info!(ctx.expect_logger(), "Running postgres migrations");
        match migrations::runner()
            .set_migration_table_name("pgmigrations")
            .run_async(&mut pg_client)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "error running pg migrations: {}",
                    e.to_string()
                );
                panic!()
            }
        };
        info!(ctx.expect_logger(), "Postgres migrations complete");
    }

    pg_client
}

pub async fn insert_rune_rows(
    rows: &Vec<DbRune>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx.prepare(
        "INSERT INTO runes
        (id, number, name, spaced_name, block_height, tx_index, tx_id, divisibility, premine, symbol, terms_amount, terms_cap,
         terms_height_start, terms_height_end, terms_offset_start, terms_offset_end, turbo, timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        ON CONFLICT (name) DO NOTHING"
    ).await.expect("Unable to prepare statement");
    for row in rows.iter() {
        match db_tx
            .execute(
                &stmt,
                &[
                    &row.id,
                    &row.number,
                    &row.name,
                    &row.spaced_name,
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
                    &row.timestamp,
                ],
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error inserting rune: {:?} {:?}", e, row
                );
                panic!()
            }
        };
    }
    Ok(true)
}

pub async fn insert_ledger_entries(
    rows: &Vec<DbLedgerEntry>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx
        .prepare(
            "INSERT INTO ledger
        (rune_id, block_height, tx_index, tx_id, output, address, amount, operation, timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .await
        .expect("Unable to prepare statement");
    for row in rows.iter() {
        match db_tx
            .execute(
                &stmt,
                &[
                    &row.rune_id,
                    &row.block_height,
                    &row.tx_index,
                    &row.tx_id,
                    &row.output,
                    &row.address,
                    &row.amount,
                    &row.operation,
                    &row.timestamp,
                ],
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error inserting ledger entry: {:?} {:?}", e, row
                );
                panic!()
            }
        };
    }
    Ok(true)
}

pub async fn pg_get_max_rune_number(client: &mut Client, _ctx: &Context) -> u32 {
    let row = client
        .query_opt("SELECT MAX(number) AS max FROM runes", &[])
        .await
        .expect("error getting max rune number");
    let Some(row) = row else {
        return 0;
    };
    let max: PgBigIntU32 = row.get("max");
    max.0
}

pub async fn pg_get_block_height(client: &mut Client, _ctx: &Context) -> Option<u64> {
    let row = client
        .query_opt("SELECT MAX(block_height) AS max FROM runes", &[])
        .await
        .expect("error getting max block height")?;
    let max: PgNumericU64 = row.get("max");
    Some(max.0)
}

pub async fn get_rune_by_rune_id(
    rune_id: &RuneId,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Option<DbRune> {
    let rows = match db_tx
        .query(
            "SELECT * FROM runes WHERE block_height = $1 AND tx_index = $2",
            &[&PgNumericU64(rune_id.block), &PgBigIntU32(rune_id.tx)],
        )
        .await
    {
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

/// Returns a `HashMap` of rune balances for a list of outputs.
pub async fn get_output_rune_balances(
    outputs: Vec<(String, u32)>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Option<HashMap<RuneId, u128>> {
    // Instead of preparing a statement and running it thousands of times.
    let mut arg_num = 1;
    let mut args = String::new();
    let mut data = vec![];
    for (tx_id, output) in outputs.iter() {
        args.push_str(format!("(${},${}),", arg_num, arg_num + 1).as_str());
        arg_num += 2;
        data.push((tx_id, PgBigIntU32(*output)));
    }
    args.pop();
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];
    for d in data.iter() {
        params.push(d.0);
        params.push(&d.1);
    }
    let rows = match db_tx
        .query(
            format!(
                "SELECT rune_id, amount FROM ledger WHERE (tx_id, output) IN ({})",
                args
            )
            .as_str(),
            &params,
        )
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            error!(
                ctx.expect_logger(),
                "error retrieving output rune balances: {}",
                e.to_string()
            );
            panic!();
        }
    };
    if rows.len() == 0 {
        return None;
    };
    let mut map = HashMap::new();
    for row in rows {
        let rune_id = RuneId::from_str(row.get("rune_id")).expect("unable to parse rune id");
        let balance: PgNumericU128 = row.get("amount");
        if let Some(entry) = map.get(&rune_id) {
            map.insert(rune_id, *entry + balance.0);
        } else {
            map.insert(rune_id, balance.0);
        }
    }
    Some(map)
}
