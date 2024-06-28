import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';
import {
  AddressParamSchema,
  LimitParamSchema,
  OffsetParamSchema,
  PaginatedResponse,
  BalanceResponseSchema,
} from '../schemas';
import { parseBalanceResponse } from '../util/helpers';

export const AddressRoutes: FastifyPluginCallback<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = (fastify, options, done) => {
  // fastify.addHook('preHandler', handleInscriptionTransfersCache);

  fastify.get(
    '/address/:address/balances',
    {
      schema: {
        operationId: 'getAddressBalances',
        summary: 'Get address balances',
        description: 'Retrieves a paginated list of address balances',
        tags: ['Runes'],
        params: Type.Object({
          address: AddressParamSchema,
        }),
        querystring: Type.Object({
          offset: Type.Optional(OffsetParamSchema),
          limit: Type.Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(BalanceResponseSchema, 'Paginated balances response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getAddressBalances(request.params.address, offset, limit);
      await reply.send({
        limit,
        offset,
        total: results.total,
        results: results.results.map(r => parseBalanceResponse(r)),
      });
    }
  );

  done();
};
