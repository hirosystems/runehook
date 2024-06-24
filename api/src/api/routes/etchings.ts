import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { Value } from '@sinclair/typebox/value';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';
import {
  EtchingActivityResponse,
  EtchingActivityResponseSchema,
  EtchingParamSchema,
  EtchingResponse,
  EtchingResponseSchema,
  LimitParamSchema,
  NotFoundResponse,
  OffsetParamSchema,
  PaginatedResponse,
} from '../schemas';
import { DbLedgerEntryWithRune, DbRune } from '../../pg/types';

function parseEtchingResponse(rune: DbRune): EtchingResponse {
  return {
    id: rune.id,
    number: rune.number,
    name: rune.name,
    spaced_name: rune.spaced_name,
    block_height: rune.block_height,
    tx_index: rune.tx_index,
    tx_id: rune.tx_id,
    divisibility: rune.divisibility,
    premine: rune.premine,
    symbol: rune.symbol,
    mint_terms: {
      amount: rune.terms_amount,
      cap: rune.terms_cap,
      height_start: rune.terms_height_start,
      height_end: rune.terms_height_end,
      offset_start: rune.terms_offset_start,
      offset_end: rune.terms_offset_end,
    },
    turbo: rune.turbo,
    minted: rune.minted,
    total_mints: rune.total_mints,
    burned: rune.burned,
    total_burns: rune.total_burns,
    timestamp: rune.timestamp,
  };
}

function parseEtchingActivityResponse(entry: DbLedgerEntryWithRune): EtchingActivityResponse {
  return {
    rune: {
      id: entry.rune_id,
      name: entry.name,
      spaced_name: entry.spaced_name,
    },
    block_height: entry.block_height,
    tx_index: entry.tx_index,
    tx_id: entry.tx_id,
    vout: entry.output,
    output: `${entry.tx_id}:${entry.output}`,
    operation: entry.operation,
    address: entry.address ?? undefined,
    receiver_address: entry.receiver_address ?? undefined,
    timestamp: entry.timestamp,
    amount: entry.amount,
  };
}

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

  done();
};
