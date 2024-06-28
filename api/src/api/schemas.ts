import { SwaggerOptions } from '@fastify/swagger';
import { Nullable, Optional, SERVER_VERSION } from '@hirosystems/api-toolkit';
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

const RuneIdSchema = Type.RegEx(/^[0-9]+:[0-9]+$/);
const RuneNameSchema = Type.RegEx(/^[A-Z]+$/);
export const RuneNameSchemaCType = TypeCompiler.Compile(RuneNameSchema);
const RuneSpacedNameSchema = Type.RegEx(/^[A-Z](•[A-Z]+)+$/);
export const RuneSpacedNameSchemaCType = TypeCompiler.Compile(RuneSpacedNameSchema);

export const EtchingParamSchema = Type.Union([RuneIdSchema, RuneNameSchema, RuneSpacedNameSchema]);
export type EtchingParam = Static<typeof EtchingParamSchema>;

export const AddressParamSchema = Type.String();
export type AddressParam = Static<typeof AddressParamSchema>;

// ==========================
// Responses
// ==========================

export const EtchingResponseSchema = Type.Object({
  id: Type.String({ examples: ['840000:1'] }),
  name: Type.String({ examples: ['ZZZZZFEHUZZZZZ'] }),
  spaced_name: Type.String({ examples: ['Z•Z•Z•Z•Z•FEHU•Z•Z•Z•Z•Z'] }),
  number: Type.Integer({ examples: [1] }),
  block_hash: Type.String({
    examples: ['00000000000000000000c9787573a1f1775a2b56b403a2d0c7957e9a5bc754bb'],
  }),
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

const RuneDetailResponseSchema = Type.Object({
  rune: Type.Object({
    id: Type.String({ examples: ['840000:1'] }),
    name: Type.String({ examples: ['ZZZZZFEHUZZZZZ'] }),
    spaced_name: Type.String({ examples: ['Z•Z•Z•Z•Z•FEHU•Z•Z•Z•Z•Z'] }),
  }),
});

export const SimpleActivityResponseSchema = Type.Object({
  block_hash: Type.String({
    examples: ['00000000000000000000c9787573a1f1775a2b56b403a2d0c7957e9a5bc754bb'],
  }),
  block_height: Type.Integer({ examples: [840000] }),
  tx_index: Type.Integer({ examples: [1] }),
  tx_id: Type.String({
    examples: ['2bb85f4b004be6da54f766c17c1e855187327112c231ef2ff35ebad0ea67c69e'],
  }),
  vout: Type.Integer({ examples: [100] }),
  output: Type.String({
    examples: ['2bb85f4b004be6da54f766c17c1e855187327112c231ef2ff35ebad0ea67c69e:100'],
  }),
  address: Optional(Type.String({ examples: ['bc1q7jd477wc5s88hsvenr0ddtatsw282hfjzg59wz'] })),
  receiver_address: Type.Optional(
    Type.String({ examples: ['bc1pgdrveee2v4ez95szaakw5gkd8eennv2dddf9rjdrlt6ch56lzrrsxgvazt'] })
  ),
  amount: Type.String({ examples: ['11000000000'] }),
  operation: Type.Union([
    Type.Literal('mint'),
    Type.Literal('burn'),
    Type.Literal('send'),
    Type.Literal('receive'),
  ]),
  timestamp: Type.Integer({ examples: [1713571767] }),
});
export type SimpleActivityResponse = Static<typeof SimpleActivityResponseSchema>;

export const ActivityResponseSchema = Type.Intersect([
  RuneDetailResponseSchema,
  SimpleActivityResponseSchema,
]);
export type ActivityResponse = Static<typeof ActivityResponseSchema>;

export const SimpleBalanceResponseSchema = Type.Object({
  address: Optional(Type.String({ examples: ['bc1q7jd477wc5s88hsvenr0ddtatsw282hfjzg59wz'] })),
  balance: Type.String({ examples: ['11000000000'] }),
});
export type SimpleBalanceResponse = Static<typeof SimpleBalanceResponseSchema>;

export const BalanceResponseSchema = Type.Intersect([
  RuneDetailResponseSchema,
  SimpleBalanceResponseSchema,
]);
export type BalanceResponse = Static<typeof BalanceResponseSchema>;

export const NotFoundResponse = Type.Object(
  {
    error: Type.Literal('Not found'),
  },
  { title: 'Not Found Response' }
);
