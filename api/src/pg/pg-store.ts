import {
  BasePgStore,
  PgConnectionVars,
  PgSqlClient,
  PgSqlQuery,
  connectPostgres,
} from '@hirosystems/api-toolkit';
import { ENV } from '../env';
import {
  DbBalance,
  DbCountedQueryResult,
  DbItemWithRune,
  DbLedgerEntry,
  DbPaginatedResult,
  DbRuneWithChainTip,
} from './types';
import {
  Address,
  BlockHeightCType,
  Block,
  Rune,
  Limit,
  Offset,
  RuneNameSchemaCType,
  RuneSpacedNameSchemaCType,
  TransactionId,
  RuneNumberSchemaCType,
} from '../api/schemas';

function runeFilter(sql: PgSqlClient, etching: string, prefix?: string): PgSqlQuery {
  const p = prefix ? `${prefix}.` : '';
  let filter = sql`${sql(`${p}id`)} = ${etching}`;
  if (RuneNameSchemaCType.Check(etching)) {
    filter = sql`${sql(`${p}name`)} = ${etching}`;
  } else if (RuneSpacedNameSchemaCType.Check(etching)) {
    filter = sql`${sql(`${p}spaced_name`)} = ${etching}`;
  } else if (RuneNumberSchemaCType.Check(etching)) {
    filter = sql`${sql(`${p}number`)} = ${etching}`;
  }
  return filter;
}

function blockFilter(sql: PgSqlClient, block: string, prefix?: string): PgSqlQuery {
  const p = prefix ? `${prefix}.` : '';
  let filter = sql`${sql(`${p}block_hash`)} = ${block}`;
  if (BlockHeightCType.Check(block)) {
    filter = sql`${sql(`${p}block_height`)} = ${block}`;
  }
  return filter;
}

export class PgStore extends BasePgStore {
  static async connect(): Promise<PgStore> {
    const pgConfig: PgConnectionVars = {
      host: ENV.PGHOST,
      port: ENV.PGPORT,
      user: ENV.PGUSER,
      password: ENV.PGPASSWORD,
      database: ENV.PGDATABASE,
    };
    const sql = await connectPostgres({
      usageName: 'runes-api-pg-store',
      connectionArgs: pgConfig,
      connectionConfig: {
        poolMax: ENV.PG_CONNECTION_POOL_MAX,
        idleTimeout: ENV.PG_IDLE_TIMEOUT,
        maxLifetime: ENV.PG_MAX_LIFETIME,
        statementTimeout: ENV.PG_STATEMENT_TIMEOUT,
      },
    });
    return new PgStore(sql);
  }

  constructor(sql: PgSqlClient) {
    super(sql);
  }

  async getChainTipEtag(): Promise<string | undefined> {
    const result = await this.sql<{ etag: string }[]>`
      SELECT block_hash AS etag FROM ledger ORDER BY block_height DESC LIMIT 1
    `;
    return result[0]?.etag;
  }

  async getEtching(id: Rune): Promise<DbRuneWithChainTip | undefined> {
    const result = await this.sql<DbRuneWithChainTip[]>`
      SELECT *, (SELECT MAX(block_height) FROM ledger) AS chain_tip
      FROM runes WHERE ${runeFilter(this.sql, id)}
    `;
    if (result.count == 0) return undefined;
    return result[0];
  }

  async getEtchings(offset: Offset, limit: Limit): Promise<DbPaginatedResult<DbRuneWithChainTip>> {
    const results = await this.sql<DbCountedQueryResult<DbRuneWithChainTip>[]>`
      WITH
        rune_count AS (SELECT COALESCE(MAX(number), 0) + 1 AS total FROM runes),
        max AS (SELECT MAX(block_height) AS max FROM ledger)
      SELECT *, (SELECT total FROM rune_count), (SELECT chain_tip FROM max)
      FROM runes
      ORDER BY block_height DESC, tx_index DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }

  private async getActivity(
    filter: PgSqlQuery,
    count: PgSqlQuery,
    offset: Offset,
    limit: Limit
  ): Promise<DbPaginatedResult<DbItemWithRune<DbLedgerEntry>>> {
    const results = await this.sql<DbCountedQueryResult<DbItemWithRune<DbLedgerEntry>>[]>`
      SELECT l.*, r.name, r.spaced_name, r.divisibility, ${count} AS total
      FROM ledger AS l
      INNER JOIN runes AS r ON r.id = l.rune_id
      WHERE ${filter}
      ORDER BY l.block_height DESC, l.tx_index DESC, l.event_index DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }

  async getRuneActivity(runeId: Rune, offset: Offset, limit: Limit) {
    return this.getActivity(
      runeFilter(this.sql, runeId, 'r'),
      this.sql`r.total_operations`,
      offset,
      limit
    );
  }

  async getRuneAddressActivity(runeId: Rune, address: Address, offset: Offset, limit: Limit) {
    return this.getActivity(
      this.sql`${runeFilter(this.sql, runeId, 'r')} AND address = ${address}`,
      this.sql`COUNT(*) OVER()`,
      offset,
      limit
    );
  }

  async getTransactionActivity(txId: TransactionId, offset: Offset, limit: Limit) {
    return this.getActivity(this.sql`l.tx_id = ${txId}`, this.sql`COUNT(*) OVER()`, offset, limit);
  }

  async getBlockActivity(block: Block, offset: Offset, limit: Limit) {
    return this.getActivity(
      blockFilter(this.sql, block, 'l'),
      this.sql`COUNT(*) OVER()`,
      offset,
      limit
    );
  }

  async getRuneHolders(
    id: Rune,
    offset: Offset,
    limit: Limit
  ): Promise<DbPaginatedResult<DbItemWithRune<DbBalance>>> {
    const results = await this.sql<DbCountedQueryResult<DbItemWithRune<DbBalance>>[]>`
      SELECT b.*, r.name, r.spaced_name, r.divisibility, COUNT(*) OVER() AS total
      FROM balances AS b
      INNER JOIN runes AS r ON r.id = b.rune_id
      WHERE ${runeFilter(this.sql, id, 'r')}
      ORDER BY b.balance DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }

  async getRuneAddressBalance(
    id: Rune,
    address: Address
  ): Promise<DbItemWithRune<DbBalance> | undefined> {
    const results = await this.sql<DbItemWithRune<DbBalance>[]>`
      SELECT b.*, r.name, r.spaced_name, r.divisibility
      FROM balances AS b
      INNER JOIN runes AS r ON r.id = b.rune_id
      WHERE ${runeFilter(this.sql, id, 'r')} AND address = ${address}
    `;
    return results[0];
  }

  async getAddressBalances(
    address: Address,
    offset: Offset,
    limit: Limit
  ): Promise<DbPaginatedResult<DbItemWithRune<DbBalance>>> {
    const results = await this.sql<DbCountedQueryResult<DbItemWithRune<DbBalance>>[]>`
      SELECT b.*, r.name, r.spaced_name, r.divisibility, COUNT(*) OVER() AS total
      FROM balances AS b
      INNER JOIN runes AS r ON r.id = b.rune_id
      WHERE address = ${address}
      ORDER BY balance DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }
}
