use std::collections::HashMap;

use bitcoin::{Address, Network, ScriptBuf};
use chainhook_sdk::types::bitcoin::TxOut;
use ordinals::{Cenotaph, Edict, Etching, Rune, RuneId, Runestone};

use crate::db::{
    models::{
        db_ledger_entry::DbLedgerEntry, db_ledger_operation::DbLedgerOperation, db_rune::DbRune,
    },
    types::pg_numeric_u128::PgNumericU128,
};

/// Holds cached data relevant to a single transaction during indexing.
pub struct TransactionCache {
    network: Network,
    pub block_height: u64,
    pub tx_index: u32,
    pub tx_id: String,
    timestamp: u32,
    /// Rune etched during this transaction
    pub etching: Option<DbRune>,
    /// Runes affected by this transaction
    runes: HashMap<RuneId, DbRune>,
    /// The output where all unallocated runes will be transferred
    pointer: u32,
    /// Holds unallocated runes premined or minted in the current transaction being processed
    unallocated_runes: HashMap<RuneId, u128>,
    /// Non-OP_RETURN outputs in this transaction
    eligible_outputs: HashMap<u32, ScriptBuf>,
    /// Total outputs contained in this transaction, including OP_RETURN outputs
    total_outputs: u32,
    /// Rune balance for each of this transaction's outputs
    output_rune_balances: HashMap<u32, HashMap<RuneId, u128>>,
}

impl TransactionCache {
    pub fn new(network: Network, block_height: u64, tx_index: u32, tx_id: &String, timestamp: u32) -> Self {
        TransactionCache {
            network,
            block_height,
            tx_index,
            tx_id: tx_id.clone(),
            timestamp,
            etching: None,
            runes: HashMap::new(),
            pointer: 0,
            unallocated_runes: HashMap::new(),
            eligible_outputs: HashMap::new(),
            total_outputs: 0,
            output_rune_balances: HashMap::new(),
        }
    }

    /// Takes this transaction's input runes and moves them to the unallocated balance for future edict allocation.
    pub fn unallocate_input_rune_balance(&mut self, input_runes: HashMap<RuneId, u128>) {
        for (rune_id, amount) in input_runes.iter() {
            self.change_unallocated_rune_balance(rune_id, *amount);
        }
    }

    /// Takes the runestone's output pointer and keeps a record of eligible outputs to send runes to.
    pub fn apply_runestone_pointer(&mut self, runestone: &Runestone, tx_outputs: &Vec<TxOut>) {
        self.total_outputs = tx_outputs.len() as u32;
        // Keep a record of non-OP_RETURN outputs.
        let mut first_eligible_output: Option<u32> = None;
        for (i, output) in tx_outputs.iter().enumerate() {
            let bytes = hex::decode(&output.script_pubkey[2..]).unwrap();
            let script = ScriptBuf::from_bytes(bytes);
            if !script.is_op_return() {
                if first_eligible_output.is_none() {
                    first_eligible_output = Some(i as u32);
                }
                self.eligible_outputs.insert(i as u32, script);
            }
        }
        if first_eligible_output.is_none() {
            todo!("burn");
        } else {
            self.pointer = runestone.pointer.unwrap_or(first_eligible_output.unwrap());
        }
    }

    /// Burns the rune balances input to this transaction.
    pub fn apply_cenotaph_input_burn(&mut self, _cenotaph: &Cenotaph) -> Vec<DbLedgerEntry> {
        let mut results = vec![];
        for (rune_id, unallocated) in self.unallocated_runes.clone().iter() {
            results.push(DbLedgerEntry::from_values(
                *unallocated,
                rune_id.clone(),
                self.block_height,
                self.tx_index,
                &self.tx_id,
                // TODO: Should this be NULL if we're burning?
                self.pointer,
                &"".to_string(),
                DbLedgerOperation::Burn,
                self.timestamp,
            ));
        }
        self.unallocated_runes.clear();
        results
    }

    /// Moves remaining unallocated runes to the correct output depending on runestone configuration. Must be called once
    /// processing for a transaction is complete.
    pub fn allocate_remaining_balances(&mut self) -> Vec<DbLedgerEntry> {
        let mut results = vec![];
        for (rune_id, unallocated) in self.unallocated_runes.clone().iter() {
            results.push(self.change_output_rune_balance(self.pointer, rune_id, *unallocated));
        }
        self.unallocated_runes.clear();
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
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.change_unallocated_rune_balance(&rune_id, etching.premine.unwrap_or(0));
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
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.change_unallocated_rune_balance(&rune_id, 0);
        (rune_id, db_rune)
    }

    pub fn apply_mint(&mut self, rune_id: &RuneId, db_rune: &DbRune) -> DbLedgerEntry {
        // TODO: What's the default mint amount if none was provided?
        let mint_amount = db_rune.terms_amount.unwrap_or(PgNumericU128(0));
        self.change_unallocated_rune_balance(rune_id, mint_amount.0);
        self.runes.insert(rune_id.clone(), db_rune.clone());
        DbLedgerEntry::from_values(
            mint_amount.0,
            rune_id.clone(),
            self.block_height,
            self.tx_index,
            &self.tx_id,
            self.pointer,
            &"".to_string(),
            DbLedgerOperation::Mint,
            self.timestamp,
        )
        // TODO: Update rune minted total and number of mints
    }

    pub fn apply_cenotaph_mint(&mut self, rune_id: &RuneId, db_rune: &DbRune) -> DbLedgerEntry {
        // TODO: What's the default mint amount if none was provided?
        let mint_amount = db_rune.terms_amount.unwrap_or(PgNumericU128(0));
        self.runes.insert(rune_id.clone(), db_rune.clone());
        DbLedgerEntry::from_values(
            mint_amount.0,
            rune_id.clone(),
            self.block_height,
            self.tx_index,
            &self.tx_id,
            // TODO: Should this be NULL if we're burning?
            self.pointer,
            &"".to_string(),
            DbLedgerOperation::Burn,
            self.timestamp,
        )
        // TODO: Update rune minted+burned total and number of mints+burns
    }

    pub fn apply_edict(&mut self, edict: &Edict, db_rune: &DbRune) -> Vec<DbLedgerEntry> {
        let rune_id = if edict.id.block == 0 && edict.id.tx == 0 {
            let Some(etching) = self.etching.as_ref() else {
                // unreachable?
                return vec![];
            };
            etching.rune_id()
        } else {
            edict.id
        };
        let Some(mut unallocated) = self.unallocated_runes.get(&rune_id).copied() else {
            // no balance to allocate?
            return vec![];
        };
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
                        results.push(self.change_output_rune_balance(
                            output,
                            &rune_id,
                            per_output + extra,
                        ));
                    }
                    unallocated = 0;
                } else {
                    // Give `amount` to all outputs or until unallocated runs out.
                    for output in output_keys {
                        let amount = edict.amount.min(unallocated);
                        results.push(self.change_output_rune_balance(output, &rune_id, amount));
                        unallocated -= amount;
                    }
                }
            }
            // Send balance to the output specified by the edict.
            output if output < self.total_outputs => {
                let mut amount = edict.amount;
                if edict.amount == 0 {
                    amount = unallocated;
                    unallocated = 0;
                }
                results.push(self.change_output_rune_balance(edict.output, &rune_id, amount));
            }
            _ => {
                // TODO: what now?
            }
        }
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.unallocated_runes.insert(rune_id.clone(), unallocated);
        results
    }

    fn change_unallocated_rune_balance(&mut self, rune_id: &RuneId, delta: u128) {
        if let Some(balance) = self.unallocated_runes.get(&rune_id).copied() {
            self.unallocated_runes
                .insert(rune_id.clone(), balance + delta);
        } else {
            self.unallocated_runes.insert(rune_id.clone(), delta);
        }
    }

    fn change_output_rune_balance(
        &mut self,
        output: u32,
        rune_id: &RuneId,
        delta: u128,
    ) -> DbLedgerEntry {
        if let Some(pointer_bal) = self.output_rune_balances.get_mut(&output) {
            if let Some(rune_bal) = pointer_bal.get(&rune_id).copied() {
                pointer_bal.insert(rune_id.clone(), rune_bal + delta);
            } else {
                pointer_bal.insert(rune_id.clone(), delta);
            }
        } else {
            let mut map = HashMap::new();
            map.insert(rune_id.clone(), delta);
            self.output_rune_balances.insert(output, map);
        }
        let Some(script) = self.eligible_outputs.get(&output) else {
            // TODO: log
            // Burn runes because pointer is invalid.
            return DbLedgerEntry::from_values(
                delta,
                rune_id.clone(),
                self.block_height,
                self.tx_index,
                &self.tx_id,
                output,
                &"".to_string(),
                DbLedgerOperation::Burn,
                self.timestamp,
            );
        };
        DbLedgerEntry::from_values(
            delta,
            rune_id.clone(),
            self.block_height,
            self.tx_index,
            &self.tx_id,
            output,
            &Address::from_script(script, self.network)
                .unwrap()
                .to_string(),
            DbLedgerOperation::Receive,
            self.timestamp,
        )
    }
}
