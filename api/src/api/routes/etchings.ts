import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { Value } from '@sinclair/typebox/value';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';
import { EtchingParamSchema, EtchingResponseSchema, NotFoundResponse } from '../schemas';

export const EtchingRoutes: FastifyPluginCallback<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = (fastify, options, done) => {
  // fastify.addHook('preHandler', handleInscriptionTransfersCache);

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
        await reply.send({
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
        });
      }
    }
  );

  done();
};
