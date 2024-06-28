import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { Value } from '@sinclair/typebox/value';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';
import {
  AddressParamSchema,
  EtchingParamSchema,
  EtchingResponseSchema,
  LimitParamSchema,
  NotFoundResponse,
  OffsetParamSchema,
  SimpleBalanceResponseSchema,
  SimpleActivityResponseSchema,
} from '../schemas';
import {
  parseBalanceResponse,
  parseEtchingActivityResponse,
  parseEtchingResponse,
} from '../util/helpers';
import { Optional, PaginatedResponse } from '@hirosystems/api-toolkit';
import { handleCache } from '../util/cache';

export const EtchingRoutes: FastifyPluginCallback<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = (fastify, options, done) => {
  fastify.addHook('preHandler', handleCache);

  fastify.get(
    '/etchings',
    {
      schema: {
        operationId: 'getEtchings',
        summary: 'Get rune etchings',
        description: 'Retrieves a paginated list of rune etchings',
        tags: ['Runes'],
        querystring: Type.Object({
          offset: Optional(OffsetParamSchema),
          limit: Optional(LimitParamSchema),
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
        operationId: 'getRuneActivity',
        summary: 'Rune rune activity',
        description: 'Retrieves all activity for a Rune',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
        }),
        querystring: Type.Object({
          offset: Optional(OffsetParamSchema),
          limit: Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(SimpleActivityResponseSchema, 'Paginated activity response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getRuneActivity(request.params.etching, offset, limit);
      await reply.send({
        limit,
        offset,
        total: results.total,
        results: results.results.map(r => parseEtchingActivityResponse(r)),
      });
    }
  );

  fastify.get(
    '/etchings/:etching/activity/:address',
    {
      schema: {
        operationId: 'getRuneAddressActivity',
        summary: 'Rune rune activity for address',
        description: 'Retrieves all activity for a Rune address',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
          address: AddressParamSchema,
        }),
        querystring: Type.Object({
          offset: Optional(OffsetParamSchema),
          limit: Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(SimpleActivityResponseSchema, 'Paginated activity response'),
        },
      },
    },
    async (request, reply) => {
      const offset = request.query.offset ?? 0;
      const limit = request.query.limit ?? 20;
      const results = await fastify.db.getRuneAddressActivity(
        request.params.etching,
        request.params.address,
        offset,
        limit
      );
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
          offset: Optional(OffsetParamSchema),
          limit: Optional(LimitParamSchema),
        }),
        response: {
          200: PaginatedResponse(SimpleBalanceResponseSchema, 'Paginated holders response'),
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

  fastify.get(
    '/etchings/:etching/holders/:address',
    {
      schema: {
        operationId: 'getRuneHolderBalance',
        summary: 'Rune holder balance',
        description: 'Retrieves holder balance for a specific Rune',
        tags: ['Runes'],
        params: Type.Object({
          etching: EtchingParamSchema,
          address: AddressParamSchema,
        }),
        response: {
          404: NotFoundResponse,
          200: SimpleBalanceResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const balance = await fastify.db.getRuneAddressBalance(
        request.params.etching,
        request.params.address
      );
      if (!balance) {
        await reply.code(404).send(Value.Create(NotFoundResponse));
      } else {
        await reply.send(parseBalanceResponse(balance));
      }
    }
  );

  done();
};
