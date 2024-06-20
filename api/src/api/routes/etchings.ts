import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { Type } from '@sinclair/typebox';
import { Value } from '@sinclair/typebox/value';
import { FastifyPluginCallback } from 'fastify';
import { Server } from 'http';

export const EtchingRoutes: FastifyPluginCallback<
  Record<never, never>,
  Server,
  TypeBoxTypeProvider
> = (fastify, options, done) => {
  // fastify.addHook('preHandler', handleInscriptionTransfersCache);

  fastify.get(
    '/etchings/:id',
    {
      schema: {
        operationId: 'getEtching',
        summary: 'Rune etching',
        description: 'Retrieves information for a Rune etching',
        tags: ['Runes'],
        // params: Type.Object({
        //   ticker: Brc20TickerParam,
        // }),
        // response: {
        //   200: Brc20TokenDetailsSchema,
        //   404: NotFoundResponse,
        // },
      },
    },
    async (request, reply) => {
      // const token = await fastify.db.brc20.getToken({ ticker: request.params.ticker });
      // if (!token) {
      //   await reply.code(404).send(Value.Create(NotFoundResponse));
      // } else {
      //   await reply.send({
      //     token: parseBrc20Tokens([token])[0],
      //     supply: parseBrc20Supply(token),
      //   });
      // }
    }
  );

  done();
};
