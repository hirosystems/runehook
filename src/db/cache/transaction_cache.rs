use std::collections::{HashMap, VecDeque};

use bitcoin::{Address, Network, ScriptBuf};
use chainhook_sdk::{types::bitcoin::TxOut, utils::Context};
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};

use crate::db::{
    models::{
        db_ledger_entry::DbLedgerEntry, db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
    },
    types::pg_numeric_u128::PgNumericU128,
};

#[derive(Debug, Clone)]
pub struct InputRuneBalance {
    /// Previous owner of this balance. If this is `None`, it means the balance was just minted or premined.
    pub address: Option<String>,
    /// How much balance was input to this transaction.
    pub amount: u128,
}

/// Holds cached data relevant to a single transaction during indexing.
pub struct TransactionCache {
    network: Network,
    pub block_height: u64,
    pub tx_index: u32,
    pub tx_id: String,
    timestamp: u32,
    /// Rune etched during this transaction
    pub etching: Option<DbRune>,
    /// The output where all unallocated runes will be transferred to.
    pointer: u32,
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
        block_height: u64,
        tx_index: u32,
        tx_id: &String,
        timestamp: u32,
    ) -> Self {
        TransactionCache {
            network,
            block_height,
            tx_index,
            tx_id: tx_id.clone(),
            timestamp,
            etching: None,
            pointer: 0,
            input_runes: HashMap::new(),
            eligible_outputs: HashMap::new(),
            total_outputs: 0,
        }
    }

    /// Takes this transaction's input runes and moves them to the unallocated balance for future edict allocation.
    pub fn set_input_rune_balances(
        &mut self,
        input_runes: HashMap<RuneId, VecDeque<InputRuneBalance>>,
    ) {
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
                warn!(
                    ctx.expect_logger(),
                    "{}: unable to decode script for output {}", self.tx_id, i
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
            warn!(
                ctx.expect_logger(),
                "{}: no eligible non-OP_RETURN output found", self.tx_id
            );
        } else {
            self.pointer = runestone.pointer.unwrap_or(first_eligible_output.unwrap());
        }
    }

    /// Burns the rune balances input to this transaction.
    pub fn apply_cenotaph_input_burn(&mut self, _cenotaph: &Cenotaph) -> Vec<DbLedgerEntry> {
        let mut results = vec![];
        for (rune_id, unallocated) in self.input_runes.iter() {
            for balance in unallocated {
                results.push(DbLedgerEntry::from_values(
                    balance.amount,
                    *rune_id,
                    self.block_height,
                    self.tx_index,
                    &self.tx_id,
                    // TODO: Should this be NULL if we're burning?
                    self.pointer,
                    balance.address.as_ref(),
                    None,
                    DbLedgerOperation::Burn,
                    self.timestamp,
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
            results.extend(move_rune_balance_to_output(
                self.network,
                self.block_height,
                &self.tx_id,
                self.tx_index,
                self.timestamp,
                self.pointer,
                rune_id,
                unallocated,
                &self.eligible_outputs,
                0,
                ctx,
            ));
        }
        self.input_runes.clear();
        results
    }

    pub fn apply_etching(&mut self, etching: &Etching, number: u32) -> (RuneId, DbRune) {
        let rune_id = RuneId {
            block: self.block_height,
            tx: self.tx_index,
        };
        let db_rune = DbRune::from_etching(
            etching,
            number,
            self.block_height,
            self.tx_index,
            &self.tx_id,
            self.timestamp,
        );
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
        (rune_id, db_rune)
    }

    pub fn apply_cenotaph_etching(&mut self, rune: &Rune, number: u32) -> (RuneId, DbRune) {
        let rune_id = RuneId {
            block: self.block_height,
            tx: self.tx_index,
        };
        // If the runestone that produced the cenotaph contained an etching, the etched rune has supply zero and is unmintable.
        let db_rune = DbRune::from_cenotaph_etching(
            rune,
            number,
            self.block_height,
            self.tx_index,
            &self.tx_id,
            self.timestamp,
        );
        self.etching = Some(db_rune.clone());
        (rune_id, db_rune)
    }

    pub fn apply_mint(&mut self, rune_id: &RuneId, db_rune: &DbRune) -> DbLedgerEntry {
        // TODO: What's the default mint amount if none was provided?
        let mint_amount = db_rune.terms_amount.unwrap_or(PgNumericU128(0));
        // TODO: Update rune minted total and number of mints
        self.add_input_runes(
            rune_id,
            InputRuneBalance {
                address: None,
                amount: mint_amount.0,
            },
        );
        DbLedgerEntry::from_values(
            mint_amount.0,
            rune_id.clone(),
            self.block_height,
            self.tx_index,
            &self.tx_id,
            self.pointer,
            None,
            None,
            DbLedgerOperation::Mint,
            self.timestamp,
        )
    }

    pub fn apply_cenotaph_mint(&mut self, rune_id: &RuneId, db_rune: &DbRune) -> DbLedgerEntry {
        // TODO: What's the default mint amount if none was provided?
        let mint_amount = db_rune.terms_amount.unwrap_or(PgNumericU128(0));
        // This entry does not go in the input runes, it gets burned immediately.
        DbLedgerEntry::from_values(
            mint_amount.0,
            rune_id.clone(),
            self.block_height,
            self.tx_index,
            &self.tx_id,
            // TODO: Should this be NULL if we're burning?
            self.pointer,
            None,
            None,
            DbLedgerOperation::Burn,
            self.timestamp,
        )
        // TODO: Update rune minted+burned total and number of mints+burns
    }

    pub fn apply_edict(&mut self, edict: &Edict, ctx: &Context) -> Vec<DbLedgerEntry> {
        // Find this rune.
        let rune_id = if edict.id.block == 0 && edict.id.tx == 0 {
            let Some(etching) = self.etching.as_ref() else {
                warn!(
                    ctx.expect_logger(),
                    "{}: attempted edict for nonexistent rune 0:0", self.tx_id
                );
                return vec![];
            };
            etching.rune_id()
        } else {
            edict.id
        };
        // Take all the available inputs for the rune we're trying to move.
        let Some(available_inputs) = self.input_runes.get_mut(&rune_id) else {
            warn!(
                ctx.expect_logger(),
                "{}: no unallocated runes {} remain for edict", self.tx_id, edict.id
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
                            self.network,
                            self.block_height,
                            &self.tx_id,
                            self.tx_index,
                            self.timestamp,
                            output,
                            &rune_id,
                            available_inputs,
                            &self.eligible_outputs,
                            per_output + extra,
                            ctx,
                        ));
                    }
                } else {
                    // Give `amount` to all outputs or until unallocated runs out.
                    for output in output_keys {
                        let amount = edict.amount.min(unallocated);
                        results.extend(move_rune_balance_to_output(
                            self.network,
                            self.block_height,
                            &self.tx_id,
                            self.tx_index,
                            self.timestamp,
                            output,
                            &rune_id,
                            available_inputs,
                            &self.eligible_outputs,
                            amount,
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
                    self.network,
                    self.block_height,
                    &self.tx_id,
                    self.tx_index,
                    self.timestamp,
                    edict.output,
                    &rune_id,
                    available_inputs,
                    &self.eligible_outputs,
                    amount,
                    ctx,
                ));
            }
            _ => {
                warn!(
                    ctx.expect_logger(),
                    "{}: edict for rune {} attempted move to nonexistent output {}",
                    self.tx_id,
                    edict.id,
                    edict.output
                );
                // TODO: Burn
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

/// Takes `amount` rune balance from `available_inputs` and moves it to `output` by generating the correct ledger entries.
/// Modifies `available_inputs` to consume balance that is already moved. If `amount` is zero, all remaining balances will be
/// transferred.
fn move_rune_balance_to_output(
    network: Network,
    block_height: u64,
    tx_id: &String,
    tx_index: u32,
    timestamp: u32,
    output: u32,
    rune_id: &RuneId,
    available_inputs: &mut VecDeque<InputRuneBalance>,
    eligible_outputs: &HashMap<u32, ScriptBuf>,
    amount: u128,
    ctx: &Context,
) -> Vec<DbLedgerEntry> {
    let mut results = vec![];
    // Who is this balance going to?
    let receiver_address = match eligible_outputs.get(&output) {
        Some(script) => match Address::from_script(script, network) {
            Ok(address) => Some(address.to_string()),
            Err(e) => {
                warn!(
                    ctx.expect_logger(),
                    "{}: unable to decode address for output {}, {}", tx_id, output, e
                );
                None
            }
        },
        None => {
            warn!(
                ctx.expect_logger(),
                "{}: attempted move to non-eligible output {}", tx_id, output
            );
            None
        }
    };
    let operation = if receiver_address.is_some() {
        DbLedgerOperation::Send
    } else {
        DbLedgerOperation::Burn
    };
    // Produce the `send` ledger entries by taking balance from the available inputs until the total amount is satisfied.
    let mut total_sent = 0;
    loop {
        let Some(input_bal) = available_inputs.pop_front() else {
            warn!(
                ctx.expect_logger(),
                "{}: move amount is not satisfied by available inputs for output {}", tx_id, output
            );
            break;
        };
        let balance_taken = if amount == 0 {
            input_bal.amount
        } else {
            input_bal.amount.min(amount - total_sent)
        };
        // Empty sender address means this balance was minted or premined, so we have no "send" entry to add.
        if let Some(sender_address) = input_bal.address.clone() {
            results.push(DbLedgerEntry::from_values(
                balance_taken,
                *rune_id,
                block_height,
                tx_index,
                tx_id,
                output,
                Some(&sender_address),
                // Depending on the logic above, this might be a normal "send" or a "burn" if the target output was not found.
                receiver_address.as_ref(),
                operation.clone(),
                timestamp,
            ));
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
    if receiver_address.is_some() {
        results.push(DbLedgerEntry::from_values(
            total_sent,
            *rune_id,
            block_height,
            tx_index,
            &tx_id,
            output,
            receiver_address.as_ref(),
            None,
            DbLedgerOperation::Receive,
            timestamp,
        ));
    }
    results
}
