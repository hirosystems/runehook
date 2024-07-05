use std::collections::{HashMap, VecDeque};

use chainhook_sdk::{types::bitcoin::TxIn, utils::Context};
use lru::LruCache;
use ordinals::RuneId;
use tokio_postgres::Transaction;

use crate::db::pg_get_input_rune_balances;

use super::transaction_cache::InputRuneBalance;

/// Takes all transaction inputs and transforms them into rune balances to be allocated for operations. Looks inside an output LRU
/// cache and the DB when there are cache misses.
pub async fn input_rune_balances_from_tx_inputs(
    tx_inputs: &Vec<TxIn>,
    tx_output_cache: &HashMap<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    output_cache: &mut LruCache<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    db_tx: &mut Transaction<'_>,
    ctx: &Context,
) -> HashMap<RuneId, VecDeque<InputRuneBalance>> {
    // Maps input index to all of its rune balances. Useful in order to keep rune inputs in order.
    let mut indexed_input_runes = HashMap::new();
    let mut cache_misses = vec![];

    // Look in both current transaction output cache and in long term LRU cache.
    for (i, input) in tx_inputs.iter().enumerate() {
        let tx_id = input.previous_output.txid.hash[2..].to_string();
        let vout = input.previous_output.vout;
        let k = (tx_id.clone(), vout);
        if let Some(map) = tx_output_cache.get(&k) {
            indexed_input_runes.insert(i as u32, map.clone());
        } else if let Some(map) = output_cache.get(&k) {
            indexed_input_runes.insert(i as u32, map.clone());
        } else {
            cache_misses.push((i as u32, tx_id, vout));
        }
    }
    // Look for cache misses in database. We don't need to `flush` the DB cache here because we've already looked in the current
    // transaction's output cache.
    if cache_misses.len() > 0 {
        let output_balances = pg_get_input_rune_balances(cache_misses, db_tx, ctx).await;
        indexed_input_runes.extend(output_balances);
    }

    let mut final_input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>> = HashMap::new();
    let mut input_keys: Vec<u32> = indexed_input_runes.keys().copied().collect();
    input_keys.sort();
    for key in input_keys.iter() {
        let input_value = indexed_input_runes.get(key).unwrap();
        for (rune_id, vec) in input_value.iter() {
            if let Some(rune) = final_input_runes.get_mut(rune_id) {
                rune.extend(vec.clone());
            } else {
                final_input_runes.insert(*rune_id, VecDeque::from(vec.clone()));
            }
        }
    }
    final_input_runes
}

/// Moves data from the current transaction's output cache to the long-term LRU output cache. Clears the tx output cache when
/// done.
pub fn move_tx_output_cache_to_output_cache(
    tx_output_cache: &mut HashMap<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
    output_cache: &mut LruCache<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>>,
) {
    for (k, tx_output_map) in tx_output_cache.iter() {
        if let Some(v) = output_cache.get_mut(&k) {
            for (rune_id, balances) in tx_output_map.iter() {
                if let Some(rune_balance) = v.get_mut(&rune_id) {
                    rune_balance.extend(balances.clone());
                } else {
                    v.insert(*rune_id, balances.clone());
                }
            }
        } else {
            output_cache.push(k.clone(), tx_output_map.clone());
        }
    }
    tx_output_cache.clear();
}

#[cfg(test)]
mod test {
    // use std::{collections::HashMap, num::NonZeroUsize, str::FromStr};

    // use chainhook_sdk::{
    //     types::{
    //         bitcoin::{OutPoint, TxIn},
    //         TransactionIdentifier,
    //     },
    //     utils::Context,
    // };
    // use lru::LruCache;
    // use ordinals::RuneId;

    // use crate::db::cache::transaction_cache::InputRuneBalance;

    // #[test]
    // fn from_output_cache() {
    //     let tx_inputs = vec![TxIn {
    //         previous_output: OutPoint {
    //             txid: TransactionIdentifier {
    //                 hash: "aea76e5ef8135851d0387074cf7672013779e4506e56122e0e698e12ede62681"
    //                     .to_string(),
    //             },
    //             vout: 2,
    //             value: 100,
    //             block_height: 848300,
    //         },
    //         script_sig: "".to_string(),
    //         sequence: 1,
    //         witness: vec![],
    //     }];
    //     let mut value = HashMap::new();
    //     value.insert(
    //         RuneId::from_str("840000:1").unwrap(),
    //         vec![InputRuneBalance {
    //             address: Some("1EDYZPvGqKzZYp6DoTtcgXwvSAkA9d9UKU".to_string()),
    //             amount: 10000,
    //         }],
    //     );
    //     let mut output_cache: LruCache<(String, u32), HashMap<RuneId, Vec<InputRuneBalance>>> =
    //         LruCache::new(NonZeroUsize::new(2).unwrap());
    //     output_cache.put(
    //         (
    //             "aea76e5ef8135851d0387074cf7672013779e4506e56122e0e698e12ede62681".to_string(),
    //             2,
    //         ),
    //         value,
    //     );
    //     let ctx = Context::empty();
    // }
}
