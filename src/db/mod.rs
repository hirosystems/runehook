use std::{collections::HashMap, str::FromStr};

use cache::transaction_cache::InputRuneBalance;
use chainhook_sdk::utils::Context;
use models::{
    db_balance_change::DbBalanceChange, db_ledger_entry::DbLedgerEntry, db_rune::DbRune,
    db_supply_change::DbSupplyChange,
};
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

pub async fn pg_connect(config: &Config, run_migrations: bool, ctx: &Context) -> Client {
    let mut pg_config = tokio_postgres::Config::new();
    pg_config
        .dbname(&config.postgres.database)
        .host(&config.postgres.host)
        .port(config.postgres.port)
        .user(&config.postgres.username);
    if let Some(password) = config.postgres.password.as_ref() {
        pg_config.password(password);
    }

    let mut pg_client: Client;
    loop {
        match pg_config.connect(NoTls).await {
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

pub async fn pg_test_client() -> Client {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=postgres password=postgres", NoTls)
            .await
            .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("test connection error: {}", e);
        }
    });
    client
}

pub async fn pg_insert_runes(
    rows: &Vec<DbRune>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx.prepare(
        "INSERT INTO runes
        (id, number, name, spaced_name, block_hash, block_height, tx_index, tx_id, divisibility, premine, symbol, terms_amount,
         terms_cap, terms_height_start, terms_height_end, terms_offset_start, terms_offset_end, turbo, timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
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
                    &row.block_hash,
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

pub async fn pg_insert_supply_changes(
    rows: &Vec<DbSupplyChange>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx
        .prepare(
            "WITH previous AS (
                SELECT * FROM supply_changes
                WHERE rune_id = $1 AND block_height <= $2
                ORDER BY block_height DESC LIMIT 1
            )
            INSERT INTO supply_changes
            (rune_id, block_height, minted, total_mints, burned, total_burns, total_operations)
            VALUES ($1, $2,
                COALESCE((SELECT minted FROM previous), 0) + $3,
                COALESCE((SELECT total_mints FROM previous), 0) + $4,
                COALESCE((SELECT burned FROM previous), 0) + $5,
                COALESCE((SELECT total_burns FROM previous), 0) + $6,
                COALESCE((SELECT total_operations FROM previous), 0) + $7)
            ON CONFLICT (rune_id, block_height) DO UPDATE SET
                minted = EXCLUDED.minted,
                total_mints = EXCLUDED.total_mints,
                burned = EXCLUDED.burned,
                total_burns = EXCLUDED.total_burns,
                total_operations = EXCLUDED.total_operations",
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
                    &row.minted,
                    &row.total_mints,
                    &row.burned,
                    &row.total_burns,
                    &row.total_operations,
                ],
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error updating rune supply: {:?} {:?}", e, row
                );
                panic!()
            }
        };
    }
    Ok(true)
}

pub async fn pg_insert_balance_changes(
    rows: &Vec<DbBalanceChange>,
    increase: bool,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let sign = if increase { "+" } else { "-" };
    let stmt = db_tx
        .prepare(
            format!(
                "WITH previous AS (
                    SELECT * FROM balance_changes
                    WHERE rune_id = $1 AND block_height <= $2 AND address = $3
                    ORDER BY block_height DESC LIMIT 1
                )
                INSERT INTO balance_changes (rune_id, block_height, address, balance, total_operations)
                VALUES ($1, $2, $3,
                    COALESCE((SELECT balance FROM previous), 0) {} $4,
                    COALESCE((SELECT total_operations FROM previous), 0) {} $5)
                ON CONFLICT (rune_id, block_height, address) DO UPDATE SET
                    balance = EXCLUDED.balance,
                    total_operations = EXCLUDED.total_operations",
                sign, sign
            )
            .as_str(),
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
                    &row.address,
                    &row.balance,
                    &row.total_operations,
                ],
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!(
                    ctx.expect_logger(),
                    "Error updating balance (increase={}): {:?} {:?}", increase, e, row
                );
                panic!()
            }
        };
    }
    Ok(true)
}

pub async fn pg_insert_ledger_entries(
    rows: &Vec<DbLedgerEntry>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Result<bool, Error> {
    let stmt = db_tx
        .prepare(
            "INSERT INTO ledger
        (rune_id, block_hash, block_height, tx_index, event_index, tx_id, output, address, receiver_address, amount, operation,
         timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .await
        .expect("Unable to prepare statement");
    for row in rows.iter() {
        match db_tx
            .execute(
                &stmt,
                &[
                    &row.rune_id,
                    &row.block_hash,
                    &row.block_height,
                    &row.tx_index,
                    &row.event_index,
                    &row.tx_id,
                    &row.output,
                    &row.address,
                    &row.receiver_address,
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
        .query_opt(
            "SELECT MAX(block_height) AS max FROM runes WHERE id <> '1:0'",
            &[],
        )
        .await
        .expect("error getting max block height")?;
    let max: Option<PgNumericU64> = row.get("max");
    if let Some(max) = max {
        Some(max.0)
    } else {
        None
    }
}

pub async fn pg_get_rune_by_id(
    id: &RuneId,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Option<DbRune> {
    let row = match db_tx
        .query_opt("SELECT * FROM runes WHERE id = $1", &[&id.to_string()])
        .await
    {
        Ok(row) => row,
        Err(e) => {
            error!(
                ctx.expect_logger(),
                "error retrieving rune: {}",
                e.to_string()
            );
            panic!();
        }
    };
    let Some(row) = row else {
        return None;
    };
    Some(DbRune::from_pg_row(&row))
}

pub async fn pg_get_rune_total_mints(
    id: &RuneId,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> Option<u128> {
    let row = match db_tx
        .query_opt(
            "SELECT total_mints FROM supply_changes WHERE rune_id = $1 ORDER BY block_height DESC LIMIT 1",
            &[&id.to_string()],
        )
        .await
    {
        Ok(row) => row,
        Err(e) => {
            error!(
                ctx.expect_logger(),
                "error retrieving rune minted total: {}",
                e.to_string()
            );
            panic!();
        }
    };
    let Some(row) = row else {
        return None;
    };
    let minted: PgNumericU128 = row.get("total_mints");
    Some(minted.0)
}

pub async fn pg_get_missed_input_rune_balances(
    outputs: Vec<(u32, String, u32)>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> HashMap<u32, HashMap<RuneId, Vec<InputRuneBalance>>> {
    // Instead of preparing a statement and running it thousands of times, pull all rows with 1 query.
    let mut arg_num = 1;
    let mut args = String::new();
    let mut data = vec![];
    for (input_index, tx_id, output) in outputs.iter() {
        args.push_str(
            format!(
                "(${}::bigint,${},${}::bigint),",
                arg_num,
                arg_num + 1,
                arg_num + 2
            )
            .as_str(),
        );
        arg_num += 3;
        data.push((PgBigIntU32(*input_index), tx_id, PgBigIntU32(*output)));
    }
    args.pop();
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];
    for d in data.iter() {
        params.push(&d.0);
        params.push(d.1);
        params.push(&d.2);
    }
    let rows = match db_tx
        .query(
            format!(
                "WITH inputs (index, tx_id, output) AS (VALUES {})
                SELECT i.index, l.rune_id, l.address, l.amount
                FROM ledger AS l
                INNER JOIN inputs AS i USING (tx_id, output)
                WHERE l.operation = 'receive'",
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
    let mut results: HashMap<u32, HashMap<RuneId, Vec<InputRuneBalance>>> = HashMap::new();
    for row in rows.iter() {
        let key: PgBigIntU32 = row.get("index");
        let rune_str: String = row.get("rune_id");
        let rune_id = RuneId::from_str(rune_str.as_str()).unwrap();
        let address: Option<String> = row.get("address");
        let amount: PgNumericU128 = row.get("amount");
        let input_bal = InputRuneBalance {
            address,
            amount: amount.0,
        };
        if let Some(input) = results.get_mut(&key.0) {
            if let Some(rune_bal) = input.get_mut(&rune_id) {
                rune_bal.push(input_bal);
            } else {
                input.insert(rune_id, vec![input_bal]);
            }
        } else {
            let mut map = HashMap::new();
            map.insert(rune_id, vec![input_bal]);
            results.insert(key.0, map);
        }
    }
    results
}
