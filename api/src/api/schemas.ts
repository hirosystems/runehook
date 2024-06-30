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

export const OffsetSchema = Type.Integer({
  minimum: 0,
  title: 'Offset',
  description: 'Result offset',
});
export type Offset = Static<typeof OffsetSchema>;

export const LimitSchema = Type.Integer({
  minimum: 1,
  maximum: 60,
  title: 'Limit',
  description: 'Results per page',
});
export type Limit = Static<typeof LimitSchema>;

const RuneIdSchema = Type.RegEx(/^[0-9]+:[0-9]+$/);
const RuneNumberSchema = Type.RegEx(/^[0-9]+$/);
export const RuneNumberSchemaCType = TypeCompiler.Compile(RuneNumberSchema);
const RuneNameSchema = Type.RegEx(/^[A-Z]+$/);
export const RuneNameSchemaCType = TypeCompiler.Compile(RuneNameSchema);
const RuneSpacedNameSchema = Type.RegEx(/^[A-Z](•[A-Z]+)+$/);
export const RuneSpacedNameSchemaCType = TypeCompiler.Compile(RuneSpacedNameSchema);

export const RuneSchema = Type.Union([
  RuneIdSchema,
  RuneNumberSchema,
  RuneNameSchema,
  RuneSpacedNameSchema,
]);
export type Rune = Static<typeof RuneSchema>;

export const AddressSchema = Type.String({
  title: 'Address',
  description: 'Bitcoin address',
  examples: ['bc1p8aq8s3z9xl87e74twfk93mljxq6alv4a79yheadx33t9np4g2wkqqt8kc5'],
});
export type Address = Static<typeof AddressSchema>;

export const TransactionIdSchema = Type.RegEx(/^[a-fA-F0-9]{64}$/, {
  title: 'Transaction ID',
  description: 'A transaction ID',
  examples: ['8f46f0d4ef685e650727e6faf7e30f23b851a7709714ec774f7909b3fb5e604c'],
});
export type TransactionId = Static<typeof TransactionIdSchema>;

export const TransactionOutputSchema = Type.RegEx(/^[a-fA-F0-9]{64}:[0-9]+$/, {
  title: 'Transaction Output',
  description: 'A transaction output',
  examples: ['8f46f0d4ef685e650727e6faf7e30f23b851a7709714ec774f7909b3fb5e604c:0'],
});
export type TransactionOutput = Static<typeof TransactionOutputSchema>;

export const BlockHeightSchema = Type.RegEx(/^[0-9]+$/, {
  title: 'Block Height',
  description: 'Bitcoin block height',
  examples: [777678],
});
export const BlockHeightCType = TypeCompiler.Compile(BlockHeightSchema);
export type BlockHeight = Static<typeof BlockHeightSchema>;

const BlockHashSchema = Type.RegEx(/^[0]{8}[a-fA-F0-9]{56}$/, {
  title: 'Block Hash',
  description: 'Bitcoin block hash',
  examples: ['0000000000000000000452773967cdd62297137cdaf79950c5e8bb0c62075133'],
});
export type BlockHash = Static<typeof BlockHashSchema>;

export const BlockSchema = Type.Union([BlockHeightSchema, BlockHashSchema]);
export type Block = Static<typeof BlockSchema>;

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
  turbo: Type.Boolean({ examples: [false] }),
  timestamp: Type.Integer({ examples: [1713571767] }),
  mint_terms: Type.Object({
    amount: Nullable(Type.String({ examples: ['100'] })),
    cap: Nullable(Type.String({ examples: ['1111111'] })),
    height_start: Nullable(Type.Integer({ examples: [840000] })),
    height_end: Nullable(Type.Integer({ examples: [1050000] })),
    offset_start: Nullable(Type.Integer({ examples: [0] })),
    offset_end: Nullable(Type.Integer({ examples: [200] })),
  }),
  supply: Type.Object({
    current: Type.String({ examples: ['11274916350'] }),
    minted: Type.String({ examples: ['274916100'] }),
    total_mints: Type.String({ examples: ['250'] }),
    mint_percentage: Type.String({ examples: ['59.4567'] }),
    mintable: Type.Boolean(),
    burned: Type.String({ examples: ['5100'] }),
    total_burns: Type.String({ examples: ['17'] }),
  }),
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
