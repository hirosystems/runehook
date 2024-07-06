use std::collections::{HashMap, VecDeque};

use bitcoin::{Address, ScriptBuf};
use chainhook_sdk::{types::bitcoin::TxIn, utils::Context};
use lru::LruCache;
use ordinals::RuneId;
use tokio_postgres::Transaction;

use crate::{
    db::{
        models::{db_ledger_entry::DbLedgerEntry, db_ledger_operation::DbLedgerOperation, db_rune::DbRune},
        pg_get_input_rune_balances,
    },
    try_info, try_warn,
};

use super::{transaction_cache::InputRuneBalance, transaction_location::TransactionLocation};

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

/// Creates a new ledger entry.
pub fn new_ledger_entry(
    location: &TransactionLocation,
    amount: Option<u128>,
    rune_id: RuneId,
    output: Option<u32>,
    address: Option<&String>,
    receiver_address: Option<&String>,
    operation: DbLedgerOperation,
    next_event_index: &mut u32,
) -> DbLedgerEntry {
    let entry = DbLedgerEntry::from_values(
        amount,
        rune_id,
        &location.block_hash,
        location.block_height,
        location.tx_index,
        *next_event_index,
        &location.tx_id,
        output,
        address,
        receiver_address,
        operation,
        location.timestamp,
    );
    *next_event_index += 1;
    entry
}

/// Takes `amount` rune balance from `available_inputs` and moves it to `output` by generating the correct ledger entries.
/// Modifies `available_inputs` to consume balance that is already moved. If `amount` is zero, all remaining balances will be
/// transferred. If `output` is `None`, the runes will be burnt.
pub fn move_rune_balance_to_output(
    location: &TransactionLocation,
    output: Option<u32>,
    rune_id: &RuneId,
    available_inputs: &mut VecDeque<InputRuneBalance>,
    eligible_outputs: &HashMap<u32, ScriptBuf>,
    amount: u128,
    next_event_index: &mut u32,
    ctx: &Context,
) -> Vec<DbLedgerEntry> {
    let mut results = vec![];
    // Who is this balance going to?
    let receiver_address = if let Some(output) = output {
        match eligible_outputs.get(&output) {
            Some(script) => match Address::from_script(script, location.network) {
                Ok(address) => Some(address.to_string()),
                Err(e) => {
                    try_warn!(
                        ctx,
                        "Unable to decode address for output {}, {} {}",
                        output,
                        e,
                        location
                    );
                    None
                }
            },
            None => {
                try_info!(
                    ctx,
                    "Attempted move to non-eligible output {}, runes will be burnt {}",
                    output,
                    location
                );
                None
            }
        }
    } else {
        None
    };
    let operation = if receiver_address.is_some() {
        DbLedgerOperation::Send
    } else {
        DbLedgerOperation::Burn
    };

    // Gather balance to be received by taking it from the available inputs until the amount to move is satisfied.
    let mut total_sent = 0;
    let mut senders = vec![];
    loop {
        // Do we still have input balance left to move?
        let Some(input_bal) = available_inputs.pop_front() else {
            break;
        };
        // Select the correct move amount.
        let balance_taken = if amount == 0 {
            input_bal.amount
        } else {
            input_bal.amount.min(amount - total_sent)
        };
        total_sent += balance_taken;
        // If the input balance came from an address, add to `Send` operations.
        if let Some(sender_address) = input_bal.address.clone() {
            senders.push((balance_taken, sender_address));
        }
        // Is there still some balance left on this input? If so, keep it for later but break the loop because we've satisfied the
        // move amount.
        if balance_taken < input_bal.amount {
            available_inputs.push_front(InputRuneBalance {
                address: input_bal.address,
                amount: input_bal.amount - balance_taken,
            });
            break;
        }
        // Have we finished moving balance?
        if total_sent == amount {
            break;
        }
    }
    // Add the "receive" entry, if applicable.
    if receiver_address.is_some() && total_sent > 0 {
        results.push(new_ledger_entry(
            location,
            Some(total_sent),
            *rune_id,
            output,
            receiver_address.as_ref(),
            None,
            DbLedgerOperation::Receive,
            next_event_index,
        ));
        try_info!(
            ctx,
            "{} {} ({}) {} {}",
            DbLedgerOperation::Receive,
            rune_id,
            total_sent,
            receiver_address.as_ref().unwrap(),
            location
        );
    }
    // Add the "send"/"burn" entries.
    for (balance_taken, sender_address) in senders.iter() {
        results.push(new_ledger_entry(
            location,
            Some(*balance_taken),
            *rune_id,
            output,
            Some(sender_address),
            receiver_address.as_ref(),
            operation.clone(),
            next_event_index,
        ));
        try_info!(
            ctx,
            "{} {} ({}) {} -> {:?} {}",
            operation,
            rune_id,
            balance_taken,
            sender_address,
            receiver_address,
            location
        );
    }
    results
}

/// Determines if a mint is valid depending on the rune's mint terms.
pub fn is_rune_mintable(db_rune: &DbRune, total_mints: u128, location: &TransactionLocation) -> bool {
    if db_rune.terms_amount.is_none() {
        return false;
    }
    if let Some(terms_cap) = db_rune.terms_cap {
        if total_mints >= terms_cap.0 {
            return false;
        }
    }
    if let Some(terms_height_start) = db_rune.terms_height_start {
        if location.block_height < terms_height_start.0 {
            return false;
        }
    }
    if let Some(terms_height_end) = db_rune.terms_height_end {
        if location.block_height > terms_height_end.0 {
            return false;
        }
    }
    if let Some(terms_offset_start) = db_rune.terms_offset_start {
        if location.block_height < db_rune.block_height.0 + terms_offset_start.0 {
            return false;
        }
    }
    if let Some(terms_offset_end) = db_rune.terms_offset_end {
        if location.block_height > db_rune.block_height.0 + terms_offset_end.0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod test {
    use std::collections::{HashMap, VecDeque};
    use test_case::test_case;

    use bitcoin::ScriptBuf;
    use chainhook_sdk::utils::Context;
    use ordinals::RuneId;

    use crate::db::{
        cache::{
            transaction_cache::InputRuneBalance, transaction_location::TransactionLocation,
            utils::move_rune_balance_to_output,
        },
        models::{db_ledger_operation::DbLedgerOperation, db_rune::DbRune}, types::{pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64},
    };

    use super::is_rune_mintable;

    #[test]
    fn receives_are_registered_first() {
        let ctx = Context::empty();
        let location = TransactionLocation {
            network: bitcoin::Network::Bitcoin,
            block_hash: "00000000000000000002c0cc73626b56fb3ee1ce605b0ce125cc4fb58775a0a9"
                .to_string(),
            block_height: 840002,
            timestamp: 0,
            tx_id: "37cd29676d626492cd9f20c60bc4f20347af9c0d91b5689ed75c05bb3e2f73ef".to_string(),
            tx_index: 2936,
        };
        let mut available_inputs = VecDeque::new();
        // An input from a previous tx
        available_inputs.push_back(InputRuneBalance {
            address: Some(
                "bc1p8zxlhgdsq6dmkzk4ammzcx55c3hfrg69ftx0gzlnfwq0wh38prds0nzqwf".to_string(),
            ),
            amount: 1000,
        });
        // A mint
        available_inputs.push_back(InputRuneBalance {
            address: None,
            amount: 1000,
        });
        let mut eligible_outputs = HashMap::new();
        eligible_outputs.insert(
            0u32,
            ScriptBuf::from_hex(
                "5120388dfba1b0069bbb0ad5eef62c1a94c46e91a3454accf40bf34b80f75e2708db",
            )
            .unwrap(),
        );
        let mut next_event_index = 0;
        let results = move_rune_balance_to_output(
            &location,
            Some(0),
            &RuneId::new(840000, 25).unwrap(),
            &mut available_inputs,
            &eligible_outputs,
            0,
            &mut next_event_index,
            &ctx,
        );

        let receive = results.get(0).unwrap();
        assert_eq!(receive.event_index.0, 0u32);
        assert_eq!(receive.operation, DbLedgerOperation::Receive);
        assert_eq!(receive.amount.unwrap().0, 2000u128);

        let send = results.get(1).unwrap();
        assert_eq!(send.event_index.0, 1u32);
        assert_eq!(send.operation, DbLedgerOperation::Send);
        assert_eq!(send.amount.unwrap().0, 1000u128);

        assert_eq!(results.len(), 2);
    }

    #[test_case(840000 => false; "early block")]
    #[test_case(840500 => false; "late block")]
    #[test_case(840150 => true; "block in window")]
    #[test_case(840100 => true; "first block")]
    #[test_case(840200 => true; "last block")]
    fn mint_block_height_terms_are_validated(block_height: u64) -> bool {
        let mut rune = DbRune::factory();
        rune.terms_height_start(Some(PgNumericU64(840100)));
        rune.terms_height_end(Some(PgNumericU64(840200)));
        let mut location = TransactionLocation::factory();
        location.block_height(block_height);
        is_rune_mintable(&rune, 0, &location)
    }

    #[test_case(840000 => false; "early block")]
    #[test_case(840500 => false; "late block")]
    #[test_case(840150 => true; "block in window")]
    #[test_case(840100 => true; "first block")]
    #[test_case(840200 => true; "last block")]
    fn mint_block_offset_terms_are_validated(block_height: u64) -> bool {
        let mut rune = DbRune::factory();
        rune.terms_offset_start(Some(PgNumericU64(100)));
        rune.terms_offset_end(Some(PgNumericU64(200)));
        let mut location = TransactionLocation::factory();
        location.block_height(block_height);
        is_rune_mintable(&rune, 0, &location)
    }

    #[test_case(0 => true; "first mint")]
    #[test_case(49 => true; "last mint")]
    #[test_case(50 => false; "out of range")]
    fn mint_cap_is_validated(cap: u128) -> bool {
        let mut rune = DbRune::factory();
        rune.terms_cap(Some(PgNumericU128(50)));
        is_rune_mintable(&rune, cap, &TransactionLocation::factory())
    }

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
