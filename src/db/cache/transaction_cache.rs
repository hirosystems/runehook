use std::{
    collections::{HashMap, VecDeque},
    vec,
};

use bitcoin::{Address, Network, ScriptBuf};
use chainhook_sdk::{types::bitcoin::TxOut, utils::Context};
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};

use crate::{
    db::models::{
        db_ledger_entry::DbLedgerEntry, db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
    },
    try_debug, try_info, try_warn,
};

use super::transaction_location::TransactionLocation;

#[derive(Debug, Clone)]
pub struct InputRuneBalance {
    /// Previous owner of this balance. If this is `None`, it means the balance was just minted or premined.
    pub address: Option<String>,
    /// How much balance was input to this transaction.
    pub amount: u128,
}

/// Holds cached data relevant to a single transaction during indexing.
pub struct TransactionCache {
    pub location: TransactionLocation,
    /// Index of the ledger entry we're inserting next for this transaction.
    next_event_index: u32,
    /// Rune etched during this transaction
    pub etching: Option<DbRune>,
    /// The output where all unallocated runes will be transferred to.
    pointer: Option<u32>,
    /// Holds input runes for the current transaction (input to this tx, premined or minted). Balances in the vector are in the
    /// order in which they were input to this transaction.
    input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>>,
    /// Non-OP_RETURN outputs in this transaction
    eligible_outputs: HashMap<u32, ScriptBuf>,
    /// Total outputs contained in this transaction, including OP_RETURN outputs
    total_outputs: u32,
}

impl TransactionCache {
    pub fn new(
        network: Network,
        block_hash: &String,
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) -> Self {
        TransactionCache {
            location: TransactionLocation {
                network,
                block_hash: block_hash.clone(),
                block_height,
                tx_id: tx_id.clone(),
                tx_index,
                timestamp,
            },
            next_event_index: 0,
            etching: None,
            pointer: None,
            input_runes: HashMap::new(),
            eligible_outputs: HashMap::new(),
            total_outputs: 0,
        }
    }

    /// Takes this transaction's input runes and moves them to the unallocated balance for future edict allocation.
    pub fn set_input_rune_balances(
        &mut self,
        input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>>,
        _ctx: &Context,
    ) {
        #[cfg(feature = "debug")]
        for (rune_id, vec) in input_runes.iter() {
            for input in vec.iter() {
                try_debug!(
                    _ctx,
                    "Input {} {:?} ({}) {}",
                    rune_id,
                    input.address,
                    input.amount,
                    self.location
                );
            }
        }
        self.input_runes = input_runes;
    }

    /// Takes the runestone's output pointer and keeps a record of eligible outputs to send runes to.
    pub fn apply_runestone_pointer(
        &mut self,
        runestone: &Runestone,
        tx_outputs: &Vec<TxOut>,
        ctx: &Context,
    ) {
        self.total_outputs = tx_outputs.len() as u32;
        // Keep a record of non-OP_RETURN outputs.
        let mut first_eligible_output: Option<u32> = None;
        for (i, output) in tx_outputs.iter().enumerate() {
            let Ok(bytes) = hex::decode(&output.script_pubkey[2..]) else {
                try_warn!(
                    ctx,
                    "Unable to decode script for output {} {}",
                    i,
                    self.location
                );
                continue;
            };
            let script = ScriptBuf::from_bytes(bytes);
            if !script.is_op_return() {
                if first_eligible_output.is_none() {
                    first_eligible_output = Some(i as u32);
                }
                self.eligible_outputs.insert(i as u32, script);
            }
        }
        if first_eligible_output.is_none() {
            try_warn!(
                ctx,
                "No eligible non-OP_RETURN output found {}",
                self.location
            );
        }
        self.pointer = if runestone.pointer.is_some() {
            runestone.pointer
        } else if first_eligible_output.is_some() {
            first_eligible_output
        } else {
            None
        };
    }

    /// Burns the rune balances input to this transaction.
    pub fn apply_cenotaph_input_burn(&mut self, _cenotaph: &Cenotaph) -> Vec<DbLedgerEntry> {
        let mut results = vec![];
        for (rune_id, unallocated) in self.input_runes.iter() {
            for balance in unallocated {
                results.push(new_ledger_entry(
                    &self.location,
                    Some(balance.amount),
                    *rune_id,
                    None,
                    balance.address.as_ref(),
                    None,
                    DbLedgerOperation::Burn,
                    &mut self.next_event_index,
                ));
            }
        }
        self.input_runes.clear();
        results
    }

    /// Moves remaining input runes to the correct output depending on runestone configuration. Must be called once the processing
    /// for a transaction is complete.
    pub fn allocate_remaining_balances(&mut self, ctx: &Context) -> Vec<DbLedgerEntry> {
        let mut results = vec![];
        for (rune_id, unallocated) in self.input_runes.iter_mut() {
            #[cfg(feature = "debug")]
            for input in unallocated.iter() {
                try_debug!(
                    ctx,
                    "Assign unallocated {} {:?} ({}) {}",
                    rune_id,
                    input.address,
                    input.amount,
                    self.location
                );
            }
            results.extend(move_rune_balance_to_output(
                &self.location,
                self.pointer,
                rune_id,
                unallocated,
                &self.eligible_outputs,
                0, // All of it
                &mut self.next_event_index,
                ctx,
            ));
        }
        self.input_runes.clear();
        results
    }

    pub fn apply_etching(
        &mut self,
        etching: &Etching,
        number: u32,
    ) -> (RuneId, DbRune, DbLedgerEntry) {
        let rune_id = self.location.rune_id();
        let db_rune = DbRune::from_etching(etching, number, &self.location);
        self.etching = Some(db_rune.clone());
        // Move pre-mined balance to input runes.
        if let Some(premine) = etching.premine {
            self.add_input_runes(
                &rune_id,
                InputRuneBalance {
                    address: None,
                    amount: premine,
                },
            );
        }
        let entry = new_ledger_entry(
            &self.location,
            None,
            rune_id,
            None,
            None,
            None,
            DbLedgerOperation::Etching,
            &mut self.next_event_index,
        );
        (rune_id, db_rune, entry)
    }

    pub fn apply_cenotaph_etching(
        &mut self,
        rune: &Rune,
        number: u32,
    ) -> (RuneId, DbRune, DbLedgerEntry) {
        let rune_id = self.location.rune_id();
        // If the runestone that produced the cenotaph contained an etching, the etched rune has supply zero and is unmintable.
        let db_rune = DbRune::from_cenotaph_etching(rune, number, &self.location);
        self.etching = Some(db_rune.clone());
        let entry = new_ledger_entry(
            &self.location,
            None,
            rune_id,
            None,
            None,
            None,
            DbLedgerOperation::Etching,
            &mut self.next_event_index,
        );
        (rune_id, db_rune, entry)
    }

    pub fn apply_mint(
        &mut self,
        rune_id: &RuneId,
        total_mints: u128,
        db_rune: &DbRune,
        ctx: &Context,
    ) -> Option<DbLedgerEntry> {
        if !is_valid_mint(db_rune, total_mints, &self.location) {
            try_debug!(ctx, "Invalid mint {} {}", rune_id, self.location);
            return None;
        }
        let terms_amount = db_rune.terms_amount.unwrap();
        try_info!(
            ctx,
            "MINT {} ({}) {} {}",
            rune_id,
            db_rune.spaced_name,
            terms_amount.0,
            self.location
        );
        self.add_input_runes(
            rune_id,
            InputRuneBalance {
                address: None,
                amount: terms_amount.0,
            },
        );
        Some(new_ledger_entry(
            &self.location,
            Some(terms_amount.0),
            rune_id.clone(),
            None,
            None,
            None,
            DbLedgerOperation::Mint,
            &mut self.next_event_index,
        ))
    }

    pub fn apply_cenotaph_mint(
        &mut self,
        rune_id: &RuneId,
        total_mints: u128,
        db_rune: &DbRune,
        ctx: &Context,
    ) -> Option<DbLedgerEntry> {
        if !is_valid_mint(db_rune, total_mints, &self.location) {
            try_debug!(ctx, "Invalid mint {} {}", rune_id, self.location);
            return None;
        }
        let terms_amount = db_rune.terms_amount.unwrap();
        try_info!(
            ctx,
            "CENOTAPH MINT {} {} {}",
            db_rune.spaced_name,
            terms_amount.0,
            self.location
        );
        // This entry does not go in the input runes, it gets burned immediately.
        Some(new_ledger_entry(
            &self.location,
            Some(terms_amount.0),
            rune_id.clone(),
            None,
            None,
            None,
            DbLedgerOperation::Burn,
            &mut self.next_event_index,
        ))
    }

    pub fn apply_edict(&mut self, edict: &Edict, ctx: &Context) -> Vec<DbLedgerEntry> {
        // Find this rune.
        let rune_id = if edict.id.block == 0 && edict.id.tx == 0 {
            let Some(etching) = self.etching.as_ref() else {
                try_warn!(
                    ctx,
                    "Attempted edict for nonexistent rune 0:0 {}",
                    self.location
                );
                return vec![];
            };
            etching.rune_id()
        } else {
            edict.id
        };
        // Take all the available inputs for the rune we're trying to move.
        let Some(available_inputs) = self.input_runes.get_mut(&rune_id) else {
            try_warn!(
                ctx,
                "No unallocated runes {} remain for edict {}",
                edict.id,
                self.location
            );
            return vec![];
        };
        // Calculate the maximum unallocated balance we can move.
        let unallocated = available_inputs
            .iter()
            .map(|b| b.amount)
            .reduce(|acc, e| acc + e)
            .unwrap_or(0);
        // Perform movements.
        let mut results = vec![];
        if self.eligible_outputs.len() == 0 {
            // No eligible outputs means burn.
            try_warn!(
                ctx,
                "No eligible outputs for edict on rune {} {}",
                edict.id,
                self.location
            );
            results.extend(move_rune_balance_to_output(
                &self.location,
                None, // This will force a burn.
                &rune_id,
                available_inputs,
                &self.eligible_outputs,
                edict.amount,
                &mut self.next_event_index,
                ctx,
            ));
        } else {
            match edict.output {
                // An edict with output equal to the number of transaction outputs allocates `amount` runes to each non-OP_RETURN
                // output in order.
                output if output == self.total_outputs => {
                    let mut output_keys: Vec<u32> = self.eligible_outputs.keys().cloned().collect();
                    output_keys.sort();
                    if edict.amount == 0 {
                        // Divide equally. If the number of unallocated runes is not divisible by the number of non-OP_RETURN outputs,
                        // 1 additional rune is assigned to the first R non-OP_RETURN outputs, where R is the remainder after dividing
                        // the balance of unallocated units of rune id by the number of non-OP_RETURN outputs.
                        let len = self.eligible_outputs.len() as u128;
                        let per_output = unallocated / len;
                        let mut remainder = unallocated % len;
                        for output in output_keys {
                            let mut extra = 0;
                            if remainder > 0 {
                                extra = 1;
                                remainder -= 1;
                            }
                            results.extend(move_rune_balance_to_output(
                                &self.location,
                                Some(output),
                                &rune_id,
                                available_inputs,
                                &self.eligible_outputs,
                                per_output + extra,
                                &mut self.next_event_index,
                                ctx,
                            ));
                        }
                    } else {
                        // Give `amount` to all outputs or until unallocated runs out.
                        for output in output_keys {
                            let amount = edict.amount.min(unallocated);
                            results.extend(move_rune_balance_to_output(
                                &self.location,
                                Some(output),
                                &rune_id,
                                available_inputs,
                                &self.eligible_outputs,
                                amount,
                                &mut self.next_event_index,
                                ctx,
                            ));
                        }
                    }
                }
                // Send balance to the output specified by the edict.
                output if output < self.total_outputs => {
                    let mut amount = edict.amount;
                    if edict.amount == 0 {
                        amount = unallocated;
                    }
                    results.extend(move_rune_balance_to_output(
                        &self.location,
                        Some(edict.output),
                        &rune_id,
                        available_inputs,
                        &self.eligible_outputs,
                        amount,
                        &mut self.next_event_index,
                        ctx,
                    ));
                }
                _ => {
                    try_warn!(
                        ctx,
                        "Edict for {} attempted move to nonexistent output {} {}",
                        edict.id,
                        edict.output,
                        self.location
                    );
                    results.extend(move_rune_balance_to_output(
                        &self.location,
                        None, // Burn.
                        &rune_id,
                        available_inputs,
                        &self.eligible_outputs,
                        edict.amount,
                        &mut self.next_event_index,
                        ctx,
                    ));
                }
            }
        }
        results
    }

    fn add_input_runes(&mut self, rune_id: &RuneId, entry: InputRuneBalance) {
        if let Some(balance) = self.input_runes.get_mut(&rune_id) {
            balance.push_back(entry);
        } else {
            let mut vec = VecDeque::new();
            vec.push_back(entry);
            self.input_runes.insert(rune_id.clone(), vec);
        }
    }
}

/// Determines if a mint is valid depending on the rune's mint terms.
fn is_valid_mint(db_rune: &DbRune, total_mints: u128, location: &TransactionLocation) -> bool {
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

/// Creates a new ledger entry.
fn new_ledger_entry(
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
fn move_rune_balance_to_output(
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
                try_warn!(
                    ctx,
                    "Attempted move to non-eligible output {} {}",
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
        let Some(input_bal) = available_inputs.pop_front() else {
            // Unallocated balance ran out.
            break;
        };
        let balance_taken = if amount == 0 {
            input_bal.amount
        } else {
            input_bal.amount.min(amount - total_sent)
        };
        // Empty sender address means this balance was minted or premined, so we have no "send" entry to add.
        if let Some(sender_address) = input_bal.address.clone() {
            senders.push((balance_taken, sender_address));
        }
        if balance_taken < input_bal.amount {
            // There's still some balance left on this input, keep it for later.
            available_inputs.push_front(InputRuneBalance {
                address: input_bal.address,
                amount: input_bal.amount - balance_taken,
            });
            break;
        }
        total_sent += balance_taken;
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

#[cfg(test)]
mod test {
    use std::collections::{HashMap, VecDeque};
    use test_case::test_case;

    use bitcoin::ScriptBuf;
    use chainhook_sdk::utils::Context;
    use ordinals::RuneId;

    use crate::db::{
        cache::transaction_location::TransactionLocation,
        models::{db_ledger_operation::DbLedgerOperation, db_rune::DbRune},
        types::{pg_numeric_u128::PgNumericU128, pg_numeric_u64::PgNumericU64},
    };

    use super::{is_valid_mint, move_rune_balance_to_output, InputRuneBalance};

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
        is_valid_mint(&rune, 0, &location)
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
        is_valid_mint(&rune, 0, &location)
    }

    #[test_case(0 => true; "first mint")]
    #[test_case(49 => true; "last mint")]
    #[test_case(50 => false; "out of range")]
    fn mint_cap_is_validated(cap: u128) -> bool {
        let mut rune = DbRune::factory();
        rune.terms_cap(Some(PgNumericU128(50)));
        is_valid_mint(&rune, cap, &TransactionLocation::factory())
    }
}
