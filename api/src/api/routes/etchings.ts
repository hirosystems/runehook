import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { Value } from '@sinclair/typebox/value';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';
import {
  BalanceResponseSchema,
  EtchingActivityResponseSchema,
  EtchingParamSchema,
  EtchingResponseSchema,
  LimitParamSchema,
  NotFoundResponse,
  OffsetParamSchema,
  PaginatedResponse,
} from '../schemas';
import {
  parseBalanceResponse,
  parseEtchingActivityResponse,
  parseEtchingResponse,
} from '../util/helpers';

export const EtchingRoutes: FastifyPluginCallback<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = (fastify, options, done) => {
  // fastify.addHook('preHandler', handleInscriptionTransfersCache);

  fastify.get(
    '/etchings',
    {
      schema: {
        operationId: 'getEtchings',
        summary: 'Get rune etchings',
        description: 'Retrieves a paginated list of rune etchings',
        tags: ['Runes'],
        querystring: Type.Object({
          offset: Type.Optional(OffsetParamSchema),
          limit: Type.Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(EtchingResponseSchema, 'Paginated etchings response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getEtchings(offset, limit);
      await reply.send({
        limit,
        offset,
        total: results.total,
        results: results.results.map(r => parseEtchingResponse(r)),
      });
    }
  );

  fastify.get(
    '/etchings/:etching',
    {
      schema: {
        operationId: 'getEtching',
        summary: 'Rune etching',
        description: 'Retrieves information for a Rune etching',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
        }),
        response: {
          200: EtchingResponseSchema,
          404: NotFoundResponse,
        },
      },
    },
    async (request, reply) => {
      const rune = await fastify.db.getEtching(request.params.etching);
      if (!rune) {
        await reply.code(404).send(Value.Create(NotFoundResponse));
      } else {
        await reply.send(parseEtchingResponse(rune));
      }
    }
  );

  fastify.get(
    '/etchings/:etching/activity',
    {
      schema: {
        operationId: 'getEtchingActivity',
        summary: 'Rune etching activity',
        description: 'Retrieves all activity for a Rune',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
        }),
        querystring: Type.Object({
          offset: Type.Optional(OffsetParamSchema),
          limit: Type.Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(EtchingActivityResponseSchema, 'Paginated etchings response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getEtchingActivity(request.params.etching, offset, limit);
      await reply.send({
        limit,
        offset,
        total: results.total,
        results: results.results.map(r => parseEtchingActivityResponse(r)),
      });
    }
  );

  fastify.get(
    '/etchings/:etching/holders',
    {
      schema: {
        operationId: 'getRuneHolders',
        summary: 'Rune holders',
        description: 'Retrieves a paginated list of holders for a Rune',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
        }),
        querystring: Type.Object({
          offset: Type.Optional(OffsetParamSchema),
          limit: Type.Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(BalanceResponseSchema, 'Paginated holders response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getRuneHolders(request.params.etching, offset, limit);
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
