import { ENV } from '../../src/env';
import { PgStore } from '../../src/pg/pg-store';
import { dropDatabase, runMigrations, startTestApiServer, TestFastifyServer } from '../helpers';

describe('Etchings', () => {
  let db: PgStore;
  let fastify: TestFastifyServer;

  beforeEach(async () => {
    ENV.PGDATABASE = 'postgres';
    db = await PgStore.connect();
    fastify = await startTestApiServer(db);
    await runMigrations(db);
  });

  afterEach(async () => {
    await fastify.close();
    await dropDatabase(db);
    await db.close();
  });

  test('displays etched rune', async () => {
    //
  });
});
