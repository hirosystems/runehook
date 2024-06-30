import BigNumber from 'bignumber.js';
import { DbBalance, DbItemWithRune, DbLedgerEntry, DbRuneWithChainTip } from '../../pg/types';
import { EtchingResponse, ActivityResponse, BalanceResponse } from '../schemas';

function divisibility(num: string, decimals: number): string {
  return new BigNumber(num).shiftedBy(-1 * decimals).toFixed(decimals);
}

export function parseEtchingResponse(rune: DbRuneWithChainTip): EtchingResponse {
  let mintable = true;
  if (
    (rune.terms_cap && BigNumber(rune.total_mints).gte(rune.terms_cap)) ||
    (rune.terms_height_start && rune.chain_tip < rune.terms_height_start) ||
    (rune.terms_height_end && rune.chain_tip > rune.terms_height_end) ||
    (rune.terms_offset_start && rune.chain_tip < rune.block_height + rune.terms_offset_start) ||
    (rune.terms_offset_end && rune.chain_tip > rune.block_height + rune.terms_offset_end)
  ) {
    mintable = false;
  }
  return {
    id: rune.id,
    number: rune.number,
    name: rune.name,
    spaced_name: rune.spaced_name,
    block_hash: rune.block_hash,
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
    supply: {
      minted: divisibility(rune.minted, rune.divisibility),
      total_mints: rune.total_mints,
      burned: divisibility(rune.burned, rune.divisibility),
      total_burns: rune.total_burns,
      mint_percentage: rune.terms_cap
        ? BigNumber(rune.total_mints).div(rune.terms_cap).times(100).toFixed(4)
        : '0.0000',
      mintable,
    },
    turbo: rune.turbo,
    timestamp: rune.timestamp,
  };
}

export function parseActivityResponse(entry: DbItemWithRune<DbLedgerEntry>): ActivityResponse {
  return {
    rune: {
      id: entry.rune_id,
      name: entry.name,
      spaced_name: entry.spaced_name,
    },
    block_hash: entry.block_hash,
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

export function parseBalanceResponse(item: DbItemWithRune<DbBalance>): BalanceResponse {
  return {
    rune: {
      id: item.rune_id,
      name: item.name,
      spaced_name: item.spaced_name,
    },
    address: item.address,
    balance: divisibility(item.balance, item.divisibility),
  };
}
