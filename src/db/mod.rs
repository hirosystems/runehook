use chainhook_sdk::utils::Context;
use models::{DbLedgerEntry, DbRune};
use ordinals::RuneId;
use refinery::embed_migrations;
use tokio_postgres::{Client, Error, NoTls, Transaction};
use types::{PgBigIntU32, PgNumericU64};

pub mod index;
pub mod index_cache;
pub mod models;
pub mod types;

embed_migrations!("migrations");

pub async fn init_db(ctx: &Context) -> Result<Client, Error> {
    let (mut client, connection) =
        tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    info!(ctx.expect_logger(), "Running postgres migrations");
    match migrations::runner()
        .set_migration_table_name("pgmigrations")
        .run_async(&mut client)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            error!(
                ctx.expect_logger(),
                "error running pg migrations: {}",
                e.to_string()
            )
        }
    };
    info!(ctx.expect_logger(), "Postgres migrations complete");

    Ok(client)
}

pub async fn insert_rune_rows(
    rows: &Vec<DbRune>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx.prepare(
        "INSERT INTO runes
        (number, name, block_height, tx_index, tx_id, divisibility, premine, symbol, terms_amount, terms_cap, terms_height_start,
         terms_height_end, terms_offset_start, terms_offset_end, turbo)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (name) DO NOTHING"
    ).await.expect("Unable to prepare statement");
    for row in rows.iter() {
        match db_tx
            .execute(
                &stmt,
                &[
                    &row.number,
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
    let stmt = db_tx.prepare(
        "INSERT INTO ledger
        (rune_number, block_height, tx_index, tx_id, address, amount, operation)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (name) DO NOTHING"
    ).await.expect("Unable to prepare statement");
    for row in rows.iter() {
        match db_tx
            .execute(
                &stmt,
                &[
                    &row.rune_number,
                    &row.block_height,
                    &row.tx_index,
                    &row.tx_id,
                    &row.address,
                    &row.amount,
                    &row.operation,
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

pub async fn get_max_rune_number(db_tx: &mut Transaction<'_>, _ctx: &Context) -> u32 {
    let rows = db_tx
        .query("SELECT MAX(number) AS max FROM runes", &[])
        .await
        .expect("error getting max rune number");
    let Some(row) = rows.get(0) else {
        return 0;
    };
    row.get("max")
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
