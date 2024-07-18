import { readdirSync } from 'fs';
import { PgStore } from '../src/pg/pg-store';
import { FastifyBaseLogger, FastifyInstance } from 'fastify';
import { IncomingMessage, Server, ServerResponse } from 'http';
import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { buildApiServer } from '../src/api/init';
import { Rune } from '../src/api/schemas';
import { DbRune } from '../src/pg/types';

export type TestFastifyServer = FastifyInstance<
  Server,
  IncomingMessage,
  ServerResponse,
  FastifyBaseLogger,
  TypeBoxTypeProvider
>;

export async function startTestApiServer(db: PgStore): Promise<TestFastifyServer> {
  return await buildApiServer({ db });
}

export async function runMigrations(db: PgStore) {
  const contents = readdirSync('../migrations');
  await db.sqlWriteTransaction(async sql => {
    for (const fileName of contents) {
      if (!fileName.endsWith('.sql')) continue;
      await db.sql.file('../migrations/' + fileName);
    }
  });
}

export async function dropDatabase(db: PgStore) {
  await db.sqlWriteTransaction(async sql => {
    // Drop all tables.
    await sql`
      DO $$ DECLARE
          r RECORD;
      BEGIN
          FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = current_schema()) LOOP
              EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
          END LOOP;
      END $$
    `;
    // Drop all types.
    await sql`
      DO $$ DECLARE
          r RECORD;
      BEGIN
          FOR r IN (SELECT typname FROM pg_type WHERE typtype = 'e' AND typnamespace = (SELECT oid FROM pg_namespace WHERE nspname = current_schema())) LOOP
              EXECUTE 'DROP TYPE IF EXISTS ' || quote_ident(r.typname) || ' CASCADE';
          END LOOP;
      END $$;
    `;
  });
}

export async function insertRune(db: PgStore, payload: DbRune): Promise<void> {
  await db.sqlWriteTransaction(async sql => {

    const {
      id,
      name,
      spaced_name,
      number,
      block_hash,
      block_height,
      tx_index,
      tx_id,
      symbol,
      cenotaph,
      terms_amount,
      terms_cap,
      terms_height_start,
      terms_height_end,
    } = payload;

    // Insert a new rune into the 'runes' table
    // Ensure the column names and types match your database schema

    // INSERT INTO runes (
    //     id, number, name, spaced_name, block_hash, block_height, tx_index, tx_id, symbol, terms_amount,
    //     terms_cap, terms_height_start, terms_height_end, timestamp
    // )
    // '1:0', 0, 'UNCOMMONGOODS', 'UNCOMMON•GOODS',
    // '0000000000000000000320283a032748cef8227873ff4872689bf23f1cda83a5', 840000, 0, '', '⧉', 1,
    // '340282366920938463463374607431768211455', 840000, 1050000, 0
    await sql`
      INSERT INTO runes (
        id, number, name, spaced_name, block_hash, block_height, tx_index, tx_id, symbol, cenotaph,
        terms_amount, terms_cap, terms_height_start, terms_height_end, timestamp
      )
      VALUES (

      ${id}, ${number}, ${sql(name)}, ${sql(spaced_name)}, ${sql(block_hash)}, ${sql(
      block_height
    )}, ${tx_index}, ${sql(tx_id)}, ${sql(symbol)}, ${cenotaph}, ${sql(terms_amount || '')}, ${sql(
      terms_cap || ''
    )}, ${terms_height_start}, ${terms_height_end}, NOW()
      )
    `;
  });
}
