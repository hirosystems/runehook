import { ENV } from '../../src/env';
import { PgStore } from '../../src/pg/pg-store';
import { DbRune } from '../../src/pg/types';
import {
  dropDatabase,
  insertRune,
  runMigrations,
  startTestApiServer,
  TestFastifyServer,
} from '../helpers';

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
    if (fastify) {
      await fastify.close();
    }

    await dropDatabase(db);
    await db.close();
  });

  test('displays etched rune', async () => {

    // '1:0', 0, 'UNCOMMONGOODS', 'UNCOMMON•GOODS',
    // '0000000000000000000320283a032748cef8227873ff4872689bf23f1cda83a5', 840000, 0, '', '⧉', 1,
    // '340282366920938463463374607431768211455', 840000, 1050000, 0
    const rune: DbRune = {
      id: '1:1',
      name: 'Sample Rune Name',
      spaced_name: 'Sample•Rune•Name',
      number: 1,
      block_hash: 'sample_block_hash',
      block_height: '0x1a',
      tx_index: 0,
      tx_id: 'sample_tx_id',
      divisibility: 8,
      premine: '1000',
      symbol: 'SRN',
      cenotaph: true,
      terms_amount: '1000000',
      terms_cap: '5000000',
      terms_height_start: null,
      terms_height_end: null,
      terms_offset_start: null,
      terms_offset_end: null,
      turbo: false,
      minted: '1000',
      total_mints: '1500',
      burned: '500',
      total_burns: '750',
      total_operations: '2000',
      timestamp: Date.now(),
    };
    // await insertRune(db, rune);
    // const runes = await fastify.inject({
    //   method: 'GET',
    //   url: '/runes/v1/etchings/',
    // });
    // console.log(runes);
    const etching = 'UNCOMMON GOODS'
    const response = await fastify.inject({
      method: 'GET',
      url: '/runes/v1/etchings/' + etching,
    });
    expect(response.statusCode).toBe(200);
  });
});
