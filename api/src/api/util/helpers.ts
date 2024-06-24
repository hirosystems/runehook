import BigNumber from 'bignumber.js';
import { DbRune, DbLedgerEntryWithRune } from '../../pg/types';
import { EtchingResponse, EtchingActivityResponse } from '../schemas';

function divisibility(num: string, decimals: number): string {
  return new BigNumber(num).shiftedBy(-1 * decimals).toFixed(decimals);
}

export function parseEtchingResponse(rune: DbRune): EtchingResponse {
  return {
    id: rune.id,
    number: rune.number,
    name: rune.name,
    spaced_name: rune.spaced_name,
    block_height: rune.block_height,
    tx_index: rune.tx_index,
    tx_id: rune.tx_id,
    divisibility: rune.divisibility,
    premine: divisibility(rune.premine, rune.divisibility),
    symbol: rune.symbol,
    mint_terms: {
      amount: rune.terms_amount ? divisibility(rune.terms_amount, rune.divisibility) : null,
      cap: rune.terms_cap ? divisibility(rune.terms_cap, rune.divisibility) : null,
      height_start: rune.terms_height_start,
      height_end: rune.terms_height_end,
      offset_start: rune.terms_offset_start,
      offset_end: rune.terms_offset_end,
    },
    turbo: rune.turbo,
    minted: divisibility(rune.minted, rune.divisibility),
    total_mints: rune.total_mints,
    burned: divisibility(rune.burned, rune.divisibility),
    total_burns: rune.total_burns,
    timestamp: rune.timestamp,
  };
}

export function parseEtchingActivityResponse(
  entry: DbLedgerEntryWithRune
): EtchingActivityResponse {
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
    amount: divisibility(entry.amount, entry.divisibility),
  };
}
