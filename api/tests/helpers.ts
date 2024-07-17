import { readdirSync } from 'fs';
import { PgStore } from '../src/pg/pg-store';
import { FastifyBaseLogger, FastifyInstance } from 'fastify';
import { IncomingMessage, Server, ServerResponse } from 'http';
import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { buildApiServer } from '../src/api/init';

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
  const contents = readdirSync('../../migrations');
  await db.sqlWriteTransaction(async sql => {
    for (const fileName of contents) {
      if (!fileName.endsWith('.sql')) continue;
      await db.sql.file(fileName);
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
