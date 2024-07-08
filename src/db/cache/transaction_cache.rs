use std::{
    collections::{HashMap, VecDeque},
    vec,
};

use bitcoin::ScriptBuf;
use chainhook_sdk::utils::Context;
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId};

use crate::{
    db::{
        cache::utils::{is_rune_mintable, new_ledger_entry},
        models::{
            db_ledger_entry::DbLedgerEntry, db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
        },
    },
    try_debug, try_info, try_warn,
};

use super::{
    input_rune_balance::InputRuneBalance, transaction_location::TransactionLocation,
    utils::move_rune_balance_to_output,
};

/// Holds cached data relevant to a single transaction during indexing.
pub struct TransactionCache {
    pub location: TransactionLocation,
    /// Sequential index of the ledger entry we're inserting next for this transaction. Will be increased with each generated
    /// entry.
    next_event_index: u32,
    /// Rune etched during this transaction, if any.
    pub etching: Option<DbRune>,
    /// The output where all unallocated runes will be transferred to. Set to the first eligible output by default but can be
    /// overridden by a Runestone.
    pub output_pointer: Option<u32>,
    /// Holds input runes for the current transaction (input to this tx, premined or minted). Balances in the vector are in the
    /// order in which they were input to this transaction.
    input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>>,
    /// Non-OP_RETURN outputs in this transaction
    eligible_outputs: HashMap<u32, ScriptBuf>,
    /// Total outputs contained in this transaction, including non-eligible outputs.
    total_outputs: u32,
}

impl TransactionCache {
    pub fn new(
        location: TransactionLocation,
        input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>>,
        eligible_outputs: HashMap<u32, ScriptBuf>,
        first_eligible_output: Option<u32>,
        total_outputs: u32,
    ) -> Self {
        TransactionCache {
            location,
            next_event_index: 0,
            etching: None,
            output_pointer: first_eligible_output,
            input_runes,
            eligible_outputs,
            total_outputs,
        }
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
            #[cfg(not(feature = "release"))]
            for input in unallocated.iter() {
                try_debug!(
                    ctx,
                    "Assign unallocated {} to pointer {:?} {:?} ({}) {}",
                    rune_id,
                    self.output_pointer,
                    input.address,
                    input.amount,
                    self.location
                );
            }
            results.extend(move_rune_balance_to_output(
                &self.location,
                self.output_pointer,
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
        if !is_rune_mintable(db_rune, total_mints, &self.location) {
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
        if !is_rune_mintable(db_rune, total_mints, &self.location) {
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
            try_info!(
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
            try_info!(
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
                    try_info!(
                        ctx,
                        "Edict for {} attempted move to nonexistent output {}, amount will be burnt {}",
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
