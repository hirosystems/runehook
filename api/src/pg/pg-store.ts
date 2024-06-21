import {
  BasePgStore,
  PgConnectionVars,
  PgSqlClient,
  connectPostgres,
} from '@hirosystems/api-toolkit';
import { ENV } from '../env';
import { DbRune } from './types';
import { EtchingParam, RuneNameSchemaCType, RuneSpacedNameSchemaCType } from '../api/schemas';

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
    let idParam = this.sql`id = ${id}`;
    if (RuneNameSchemaCType.Check(id)) {
      idParam = this.sql`name = ${id}`;
    } else if (RuneSpacedNameSchemaCType.Check(id)) {
      idParam = this.sql`spaced_name = ${id}`;
    }
    const result = await this.sql<DbRune[]>`
      SELECT * FROM runes WHERE ${idParam}
    `;
    if (result.count == 0) return undefined;
    return result[0];
  }
}
