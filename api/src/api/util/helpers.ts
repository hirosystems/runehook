import BigNumber from 'bignumber.js';
import { DbBalance, DbItemWithRune, DbLedgerEntry, DbRuneWithChainTip } from '../../pg/types';
import { EtchingResponse, ActivityResponse, BalanceResponse } from '../schemas';

function divisibility(num: string | BigNumber, decimals: number): string {
  return new BigNumber(num).shiftedBy(-1 * decimals).toFixed(decimals);
}

export function parseEtchingResponse(rune: DbRuneWithChainTip): EtchingResponse {
  let mintable = true;
  if (
    rune.terms_amount == null ||
    (rune.terms_cap && BigNumber(rune.total_mints).gte(rune.terms_cap)) ||
    (rune.terms_height_start && BigNumber(rune.chain_tip).lt(rune.terms_height_start)) ||
    (rune.terms_height_end && BigNumber(rune.chain_tip).gt(rune.terms_height_end)) ||
    (rune.terms_offset_start &&
      BigNumber(rune.chain_tip).lt(BigNumber(rune.block_height).plus(rune.terms_offset_start))) ||
    (rune.terms_offset_end &&
      BigNumber(rune.chain_tip).gt(BigNumber(rune.block_height).plus(rune.terms_offset_end)))
  ) {
    mintable = false;
  }
  return {
    id: rune.id,
    number: rune.number,
    name: rune.name,
    spaced_name: rune.spaced_name,
    block_hash: rune.block_hash,
    block_height: parseInt(rune.block_height),
    tx_index: rune.tx_index,
    tx_id: rune.tx_id,
    divisibility: rune.divisibility,
    premine: divisibility(rune.premine, rune.divisibility),
    symbol: rune.symbol,
    mint_terms: {
      amount: rune.terms_amount ? divisibility(rune.terms_amount, rune.divisibility) : null,
      cap: rune.terms_cap ? divisibility(rune.terms_cap, rune.divisibility) : null,
      height_start: rune.terms_height_start ? parseInt(rune.terms_height_start) : null,
      height_end: rune.terms_height_end ? parseInt(rune.terms_height_end) : null,
      offset_start: rune.terms_offset_start ? parseInt(rune.terms_offset_start) : null,
      offset_end: rune.terms_offset_end ? parseInt(rune.terms_offset_end) : null,
    },
    supply: {
      current: divisibility(
        BigNumber(rune.minted).plus(rune.burned).plus(rune.premine),
        rune.divisibility
      ),
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
    block_height: parseInt(entry.block_height),
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
