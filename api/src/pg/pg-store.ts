import {
  BasePgStore,
  PgConnectionVars,
  PgSqlClient,
  PgSqlQuery,
  connectPostgres,
} from '@hirosystems/api-toolkit';
import { ENV } from '../env';
import { DbCountedQueryResult, DbLedgerEntryWithRune, DbPaginatedResult, DbRune } from './types';
import {
  EtchingParam,
  LimitParam,
  OffsetParam,
  RuneNameSchemaCType,
  RuneSpacedNameSchemaCType,
} from '../api/schemas';

function getEtchingIdWhereCondition(sql: PgSqlClient, id: string, prefix?: string): PgSqlQuery {
  const p = prefix ? `${prefix}.` : '';
  let idParam = sql`${sql(`${p}id`)} = ${id}`;
  if (RuneNameSchemaCType.Check(id)) {
    idParam = sql`${sql(`${p}name`)} = ${id}`;
  } else if (RuneSpacedNameSchemaCType.Check(id)) {
    idParam = sql`${sql(`${p}spaced_name`)} = ${id}`;
  }
  return idParam;
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

  async getEtching(id: EtchingParam): Promise<DbRune | undefined> {
    const result = await this.sql<DbRune[]>`
      SELECT * FROM runes WHERE ${getEtchingIdWhereCondition(this.sql, id)}
    `;
    if (result.count == 0) return undefined;
    return result[0];
  }

  async getEtchings(offset: OffsetParam, limit: LimitParam): Promise<DbPaginatedResult<DbRune>> {
    const results = await this.sql<DbCountedQueryResult<DbRune>[]>`
      WITH rune_count AS (SELECT COALESCE(MAX(number), 0) + 1 AS total FROM runes)
      SELECT *, (SELECT total FROM rune_count)
      FROM runes
      ORDER BY block_height DESC, tx_index DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }

  async getEtchingActivity(
    id: EtchingParam,
    offset: OffsetParam,
    limit: LimitParam
  ): Promise<DbPaginatedResult<DbLedgerEntryWithRune>> {
    const results = await this.sql<DbCountedQueryResult<DbLedgerEntryWithRune>[]>`
      SELECT l.*, r.name, r.spaced_name, COUNT(*) OVER() AS total
      FROM ledger AS l
      INNER JOIN runes AS r ON r.id = l.rune_id
      WHERE ${getEtchingIdWhereCondition(this.sql, id)}
      ORDER BY l.block_height DESC, l.tx_index DESC
      OFFSET ${offset} LIMIT ${limit}
    `;
    return {
      total: results[0]?.total ?? 0,
      results,
    };
  }
}
