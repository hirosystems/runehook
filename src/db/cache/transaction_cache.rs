use std::collections::HashMap;

use bitcoin::{Address, ScriptBuf};
use chainhook_sdk::types::bitcoin::TxOut;
use ordinals::{Edict, Etching, Rune, RuneId, Runestone};

use crate::db::{
    models::{DbLedgerEntry, DbLedgerOperation, DbRune},
    types::{PgBigIntU32, PgNumericU128, PgNumericU64},
};

use super::db_cache::DbCache;

/// Holds cached data relevant to a single transaction during indexing.
pub struct TransactionCache {
    block_height: u64,
    tx_index: u32,
    tx_id: String,
    /// Rune etched during this transaction
    pub etching: Option<DbRune>,
    /// Runes affected by this transaction
    runes: HashMap<RuneId, DbRune>,
    /// The output where all unallocated runes will be transferred
    pointer: Option<u32>,
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
    pub fn new(block_height: u64, tx_index: u32, tx_id: &String) -> Self {
        TransactionCache {
            block_height,
            tx_index,
            tx_id: tx_id.clone(),
            etching: None,
            runes: HashMap::new(),
            pointer: None,
            unallocated_runes: HashMap::new(),
            eligible_outputs: HashMap::new(),
            total_outputs: 0,
            output_rune_balances: HashMap::new(),
        }
    }

    pub fn apply_runestone_pointer(&mut self, runestone: &Runestone, tx_outputs: &Vec<TxOut>) {
        self.pointer = runestone.pointer;
        self.total_outputs = tx_outputs.len() as u32;
        // Keep a record of non-OP_RETURN outputs.
        for (i, output) in tx_outputs.iter().enumerate() {
            let bytes = hex::decode(&output.script_pubkey).unwrap();
            let script = ScriptBuf::from_bytes(bytes);
            if !script.is_op_return() {
                self.eligible_outputs.insert(i as u32, script);
            }
        }
    }

    /// Moves remaining unallocated runes to the correct output depending on runestone configuration. Must be called once
    /// processing for a transaction is complete.
    pub fn allocate_remaining_balances(&mut self, db_cache: &mut DbCache) {
        let output = self.pointer.unwrap_or(0);
        for (rune_id, unallocated) in self.unallocated_runes.clone().iter() {
            let Some(db_rune) = self.runes.get(rune_id).cloned() else {
                // log
                continue;
            };
            self.change_output_rune_balance(output, rune_id, &db_rune, *unallocated, db_cache);
        }
        self.unallocated_runes.clear();
    }

    pub fn apply_etching(
        &mut self,
        etching: &Etching,
        number: u32,
        db_cache: &mut DbCache,
    ) -> (RuneId, DbRune) {
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
        );
        db_cache.runes.push(db_rune.clone());
        self.etching = Some(db_rune.clone());
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.change_unallocated_rune_balance(&rune_id, etching.premine.unwrap_or(0));
        (rune_id, db_rune)
    }

    pub fn apply_cenotaph_etching(
        &mut self,
        rune: &Rune,
        number: u32,
        db_cache: &mut DbCache,
    ) -> (RuneId, DbRune) {
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
        );
        db_cache.runes.push(db_rune.clone());
        self.etching = Some(db_rune.clone());
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.change_unallocated_rune_balance(&rune_id, 0);
        (rune_id, db_rune)
    }

    pub fn apply_mint(&mut self, rune_id: &RuneId, db_rune: &DbRune, db_cache: &mut DbCache) {
        // TODO: What's the default mint amount if none was provided?
        let mint_amount = db_rune.terms_amount.unwrap_or(PgNumericU128(0));
        self.change_unallocated_rune_balance(rune_id, mint_amount.0);
        self.runes.insert(rune_id.clone(), db_rune.clone());
        db_cache.ledger_entries.push(DbLedgerEntry::from_values(
            mint_amount.0,
            db_rune.number.0,
            self.block_height,
            self.tx_index,
            &self.tx_id,
            &"".to_string(),
            DbLedgerOperation::Mint,
        ));
        // TODO: Update rune minted total and number of mints
    }

    pub fn apply_edict(&mut self, edict: &Edict, db_rune: &DbRune, db_cache: &mut DbCache) {
        let rune_id = if edict.id.block == 0 && edict.id.tx == 0 {
            let Some(etching) = self.etching.as_ref() else {
                // unreachable?
                return;
            };
            etching.rune_id()
        } else {
            edict.id
        };
        let Some(mut unallocated) = self.unallocated_runes.get(&rune_id).copied() else {
            // no balance to allocate?
            return;
        };
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
                        self.change_output_rune_balance(
                            output,
                            &rune_id,
                            db_rune,
                            per_output + extra,
                            db_cache,
                        );
                    }
                    unallocated = 0;
                } else {
                    // Give `amount` to all outputs or until unallocated runs out.
                    for output in output_keys {
                        let amount = edict.amount.min(unallocated);
                        self.change_output_rune_balance(
                            output, &rune_id, db_rune, amount, db_cache,
                        );
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
                self.change_output_rune_balance(edict.output, &rune_id, db_rune, amount, db_cache);
            }
            _ => {
                // TODO: what now?
            }
        }
        self.runes.insert(rune_id.clone(), db_rune.clone());
        self.unallocated_runes.insert(rune_id.clone(), unallocated);
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
        db_rune: &DbRune,
        delta: u128,
        db_cache: &mut DbCache,
    ) {
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
        let script = self.eligible_outputs.get(&output).unwrap();
        db_cache.ledger_entries.push(DbLedgerEntry {
            rune_number: db_rune.number,
            block_height: PgNumericU64(self.block_height),
            tx_index: PgBigIntU32(self.tx_index),
            tx_id: self.tx_id.clone(),
            address: Address::from_script(script, bitcoin::Network::Bitcoin)
                .unwrap()
                .to_string(),
            amount: PgNumericU128(delta),
            operation: DbLedgerOperation::Receive,
        });
    }
}
