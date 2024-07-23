import { ENV } from '../../src/env';
import { PgStore } from '../../src/pg/pg-store';
import { DbLedgerEntry } from '../../src/pg/types';
import {
  dropDatabase,
  insertDbEntry,
  insertRune,
  sampleRune,
  runMigrations,
  startTestApiServer,
  TestFastifyServer,
  insertSupply,
  sampleLedgerEntry,
} from '../helpers';

describe('Etchings', () => {
  let db: PgStore;
  let fastify: TestFastifyServer;

  const rune = sampleRune('1:1', 'Sample Rune');
  const ledgerEntry = sampleLedgerEntry(rune.id);

  beforeEach(async () => {
    ENV.PGDATABASE = 'postgres';
    db = await PgStore.connect();
    fastify = await startTestApiServer(db);
    await runMigrations(db);
    await insertRune(db, rune);
    const event_index = 0;
    await insertDbEntry(db, ledgerEntry, event_index);
    await insertSupply(db, rune.id, 1);
  });

  afterEach(async () => {
    if (fastify) {
      await fastify.close();
    }

    await dropDatabase(db);
    await db.close();
  });

  test('lists runes', async () => {
    const runesResponse = await fastify.inject({
      method: 'GET',
      url: '/runes/v1/etchings',
    });
    expect(runesResponse.statusCode).toBe(200);
    expect(runesResponse.json().results).not.toHaveLength(0);

    const response = await fastify.inject({
      method: 'GET',
      url: '/runes/v1/etchings/' + ledgerEntry.rune_id,
    });
    expect(response.statusCode).toBe(200);
    expect(response.json().name).toEqual(rune.name);
  });
  test('can fetch by spaced name', async () => {
    const url = '/runes/v1/etchings/' + rune.spaced_name;
    const response = await fastify.inject({
      method: 'GET',
      url: url,
    });
    expect(response.statusCode).toBe(200);
    expect(response.json().spaced_name).toEqual(rune.spaced_name);
  });
  test('can not fetch by spaced name if lacking bullets', async () => {
    const url = '/runes/v1/etchings/' + rune.spaced_name.replaceAll('•', '-');
    const response = await fastify.inject({
      method: 'GET',
      url: url,
    });
    expect(response.statusCode).toBe(400);
  });
});
