import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { buildFastifyApiServer } from '@hirosystems/api-toolkit';
import { FastifyPluginAsync } from 'fastify';
import { Server } from 'http';
import { PgStore } from '../pg/pg-store';
import { EtchingRoutes } from './routes/etchings';
import { AddressRoutes } from './routes/addresses';
import { TransactionRoutes } from './routes/transactions';
import { BlockRoutes } from './routes/blocks';
import { StatusRoutes } from './routes/status';

export const Api: FastifyPluginAsync<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = async fastify => {
  await fastify.register(StatusRoutes);
  await fastify.register(EtchingRoutes);
  await fastify.register(AddressRoutes);
  await fastify.register(TransactionRoutes);
  await fastify.register(BlockRoutes);
};

export async function buildApiServer(args: { db: PgStore }) {
  const fastify = await buildFastifyApiServer();

  fastify.decorate('db', args.db);
  await fastify.register(Api, { prefix: '/runes/v1' });
  await fastify.register(Api, { prefix: '/runes' });

  return fastify;
}
