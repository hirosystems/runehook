import { SwaggerOptions } from '@fastify/swagger';
import { SERVER_VERSION } from '@hirosystems/api-toolkit';
import { Static, TSchema, Type } from '@sinclair/typebox';
import { TypeCompiler } from '@sinclair/typebox/compiler';

export const OpenApiSchemaOptions: SwaggerOptions = {
  openapi: {
    info: {
      title: 'Runes API',
      description: ``,
      version: SERVER_VERSION.tag,
    },
    externalDocs: {
      url: 'https://github.com/hirosystems/runehook',
      description: 'Source Repository',
    },
    servers: [
      {
        url: 'https://api.hiro.so/',
        description: 'mainnet',
      },
    ],
    tags: [
      {
        name: 'Runes',
        description: '',
      },
    ],
  },
};

const Nullable = <T extends TSchema>(type: T) => Type.Union([type, Type.Null()]);

// ==========================
// Parameters
// ==========================

export const OffsetParamSchema = Type.Integer({
  minimum: 0,
  title: 'Offset',
  description: 'Result offset',
});
export type OffsetParam = Static<typeof OffsetParamSchema>;

export const LimitParamSchema = Type.Integer({
  minimum: 1,
  maximum: 60,
  title: 'Limit',
  description: 'Results per page',
});
export type LimitParam = Static<typeof LimitParamSchema>;

export const RuneIdSchema = Type.RegEx(/^[0-9]+:[0-9]+$/);
export const RuneIdSchemaCType = TypeCompiler.Compile(RuneIdSchema);
export const RuneNameSchema = Type.RegEx(/^[A-Z]+$/);
export const RuneNameSchemaCType = TypeCompiler.Compile(RuneNameSchema);
export const RuneSpacedNameSchema = Type.RegEx(/^[A-Z](•[A-Z]+)+$/);
export const RuneSpacedNameSchemaCType = TypeCompiler.Compile(RuneSpacedNameSchema);

export const EtchingParamSchema = Type.Union([RuneIdSchema, RuneNameSchema, RuneSpacedNameSchema]);
export type EtchingParam = Static<typeof EtchingParamSchema>;

// ==========================
// Responses
// ==========================

export const PaginatedResponse = <T extends TSchema>(type: T, title: string) =>
  Type.Object(
    {
      limit: Type.Integer({ examples: [20] }),
      offset: Type.Integer({ examples: [0] }),
      total: Type.Integer({ examples: [1] }),
      results: Type.Array(type),
    },
    { title }
  );

export const EtchingResponseSchema = Type.Object({
  id: Type.String({ examples: ['840000:1'] }),
  name: Type.String({ examples: ['ZZZZZFEHUZZZZZ'] }),
  spaced_name: Type.String({ examples: ['Z•Z•Z•Z•Z•FEHU•Z•Z•Z•Z•Z'] }),
  number: Type.Integer({ examples: [1] }),
  block_height: Type.Integer({ examples: [840000] }),
  tx_index: Type.Integer({ examples: [1] }),
  tx_id: Type.String({
    examples: ['2bb85f4b004be6da54f766c17c1e855187327112c231ef2ff35ebad0ea67c69e'],
  }),
  divisibility: Type.Integer({ examples: [2] }),
  premine: Type.String({ examples: ['11000000000'] }),
  symbol: Type.String({ examples: ['ᚠ'] }),
  mint_terms: Type.Object({
    amount: Nullable(Type.String({ examples: ['100'] })),
    cap: Nullable(Type.String({ examples: ['1111111'] })),
    height_start: Nullable(Type.Integer({ examples: [840000] })),
    height_end: Nullable(Type.Integer({ examples: [1050000] })),
    offset_start: Nullable(Type.Integer({ examples: [0] })),
    offset_end: Nullable(Type.Integer({ examples: [200] })),
  }),
  turbo: Type.Boolean({ examples: [false] }),
  minted: Type.String({ examples: ['274916100'] }),
  total_mints: Type.Integer({ examples: [250] }),
  burned: Type.String({ examples: ['5100'] }),
  total_burns: Type.Integer({ examples: [17] }),
  timestamp: Type.Integer({ examples: [1713571767] }),
});
export type EtchingResponse = Static<typeof EtchingResponseSchema>;

export const NotFoundResponse = Type.Object(
  {
    error: Type.Literal('Not found'),
  },
  { title: 'Not Found Response' }
);
