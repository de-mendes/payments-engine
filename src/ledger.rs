use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    ChargeBack,
}

impl Display for TransactionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionType::Deposit => write!(f, "deposit"),
            TransactionType::Withdrawal => write!(f, "withdrawal"),
            TransactionType::Dispute => write!(f, "dispute"),
            TransactionType::Resolve => write!(f, "resolve"),
            TransactionType::ChargeBack => write!(f, "chargeback"),
        }
    }
}

impl TransactionType {
    fn check_state_transition(&self, previous_state: &TransactionType) -> Result<(), String> {
        match (self, previous_state) {
            (TransactionType::Dispute, TransactionType::Deposit) => Ok(()),
            (TransactionType::ChargeBack, TransactionType::Dispute) => Ok(()),
            (TransactionType::Resolve, TransactionType::Dispute) => Ok(()),
            _ => Err(format!(
                "Invalid state transition from '{}' to '{}'",
                previous_state, self
            )),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Transaction {
    #[serde(rename(deserialize = "type"))]
    transaction_type: TransactionType,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct AccountStatus {
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

#[derive(Debug)]
struct TransactionInformation {
    client: u16,
    amount: Decimal,
    state: TransactionType,
}

pub struct Ledger {
    transactions: HashMap<u32, TransactionInformation>,
    client_accounts: HashMap<u16, AccountStatus>,
}

impl Ledger {
    pub fn new() -> Self {
        Ledger {
            transactions: HashMap::new(),
            client_accounts: HashMap::new(),
        }
    }

    fn handle_transaction_transition(
        &mut self,
        tx_id: u32,
        client_id: u16,
        new_state: TransactionType,
    ) -> Result<&TransactionInformation, String> {
        let Some(tx) = self.transactions.get_mut(&tx_id) else {
            return Err(format!("Transaction with id {} does not exist", tx_id));
        };

        if tx.client != client_id {
            return Err(format!(
                "Transaction with id {} does not correspond to client with id '{}'",
                tx_id, client_id
            ));
        };
        new_state.check_state_transition(&tx.state)?;

        tx.state = new_state;

        Ok(tx)
    }

    fn check_account_is_locked(&self, client_id: u16) -> Result<(), String> {
        if let Some(account) = self.client_accounts.get(&client_id)
            && account.locked
        {
            return Err(format!("Account of client with id {} is locked", client_id));
        }
        Ok(())
    }

    fn store_new_transaction(
        &mut self,
        tx_id: u32,
        amount: Decimal,
        client_id: u16,
    ) -> Result<(), String> {
        // Assumption or comes in the document?
        if let Some(_) = self.transactions.get(&tx_id) {
            return Err(format!(
                "Cannot process a deposit with a duplicated transaction id {} ",
                tx_id
            ));
        }

        self.transactions.insert(
            tx_id,
            TransactionInformation {
                client: client_id,
                amount,
                state: TransactionType::Deposit,
            },
        );

        Ok(())
    }

    pub(crate) fn handle_new_transaction(&mut self, transaction: &Transaction) -> Result<(), String> {
        match transaction.transaction_type {
            TransactionType::Deposit => {
                let Some(amount) = transaction.amount else {
                    return Err(
                        "<Deposits must have an amount. Input CSV format is wrong>".to_string()
                    );
                };
                if let Some(account) = self.client_accounts.get_mut(&transaction.client) {
                    if account.locked {
                        return Err(format!(
                            "Account of client with id {} is locked",
                            transaction.client
                        ));
                    }
                    account.available += amount;
                    account.total += amount;
                } else {
                    self.client_accounts.insert(
                        transaction.client,
                        AccountStatus {
                            available: amount,
                            held: Decimal::zero(),
                            total: amount,
                            locked: false,
                        },
                    );
                }
                self.store_new_transaction(transaction.tx, amount, transaction.client)?
            }
            TransactionType::Withdrawal => {
                let Some(amount) = transaction.amount else {
                    return Err(
                        "<Withdrawals must have an amount. Input CSV format is wrong>".to_string(),
                    );
                };
                if let Some(_) = self.transactions.get(&transaction.tx) {
                    return Err(format!(
                        "Cannot process a withdrawal with a duplicated transaction id {} ",
                        transaction.tx
                    ));
                }
                if let Some(account) = self.client_accounts.get_mut(&transaction.client) {
                    if account.locked {
                        return Err(format!(
                            "Account of client with id {} is locked",
                            transaction.client
                        ));
                    }
                    if account.available < amount {
                        return Err(format!(
                            "Unable to process the withdrawal of {} for client with id {}: available funds {}",
                            amount, transaction.client, account.available,
                        ));
                    }
                    account.available -= amount;
                    account.total -= amount;
                }
            }
            TransactionType::Dispute => {
                self.check_account_is_locked(transaction.client)?;

                let amount = {
                    let tx = self.handle_transaction_transition(
                        transaction.tx,
                        transaction.client,
                        transaction.transaction_type,
                    )?;
                    tx.amount
                };

                if let Some(account) = self.client_accounts.get_mut(&transaction.client)
                    && account.available >= amount
                {
                    account.held += amount;
                    account.available -= amount;
                }
            }
            TransactionType::Resolve => {
                self.check_account_is_locked(transaction.client)?;

                let amount = {
                    let tx = self.handle_transaction_transition(
                        transaction.tx,
                        transaction.client,
                        transaction.transaction_type,
                    )?;
                    tx.amount
                };

                if let Some(account) = self.client_accounts.get_mut(&transaction.client)
                    && account.held >= amount
                {
                    account.held -= amount;
                    account.available += amount;
                }
            }
            TransactionType::ChargeBack => {
                self.check_account_is_locked(transaction.client)?;

                let amount = {
                    let tx = self.handle_transaction_transition(
                        transaction.tx,
                        transaction.client,
                        transaction.transaction_type,
                    )?;
                    tx.amount
                };

                if let Some(account) = self.client_accounts.get_mut(&transaction.client) {
                    if account.held >= amount {
                        account.held -= amount;
                        account.total -= amount;
                    }
                    account.locked = true;
                }

                // Assumption: Charged back transactions are not required in the future.
                self.transactions.remove(&transaction.tx);
            }
        }
        Ok(())
    }

    pub fn client_accounts(&self) -> &HashMap<u16, AccountStatus> {
        &self.client_accounts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn check_state_transitions() {
        // Valid transitions
        assert!(TransactionType::Dispute
            .check_state_transition(&TransactionType::Deposit)
            .is_ok());
        assert!(TransactionType::Resolve
            .check_state_transition(&TransactionType::Dispute)
            .is_ok());
        assert!(TransactionType::ChargeBack
            .check_state_transition(&TransactionType::Dispute)
            .is_ok());

        // Invalid transitions
        let result = TransactionType::Resolve.check_state_transition(&TransactionType::Deposit);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid state transition"));

        assert!(TransactionType::ChargeBack
            .check_state_transition(&TransactionType::Deposit)
            .is_err());

        assert!(TransactionType::Dispute
            .check_state_transition(&TransactionType::Withdrawal)
            .is_err());

        assert!(TransactionType::Dispute
            .check_state_transition(&TransactionType::Resolve)
            .is_err());
    }

    #[test]
    fn check_account() {
        let mut ledger = Ledger::new();
        let client_id: u16 = 1;

        // Account doesn't exist
        assert!(ledger.check_account_is_locked(client_id).is_ok());

        // Account exists but is not locked
        ledger.client_accounts.insert(
            client_id,
            AccountStatus {
                available: Decimal::from_str("100").unwrap(),
                held: Decimal::zero(),
                total: Decimal::from_str("100").unwrap(),
                locked: false,
            },
        );
        assert!(ledger.check_account_is_locked(client_id).is_ok());

        // Account exists and is locked
        ledger.client_accounts.get_mut(&client_id).unwrap().locked = true;
        let result = ledger.check_account_is_locked(client_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is locked"));
    }

    #[test]
    fn handle_transaction_transition_tx_not_found() {
        let mut ledger = Ledger::new();

        let result = ledger.handle_transaction_transition(1, 1, TransactionType::Dispute);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn handle_transaction_transition_wrong_client() {
        let mut ledger = Ledger::new();
        let tx_id: u32 = 1;
        let original_client: u16 = 1;
        let wrong_client: u16 = 2;

        ledger.transactions.insert(
            tx_id,
            TransactionInformation {
                client: original_client,
                amount: Decimal::from_str("50").unwrap(),
                state: TransactionType::Deposit,
            },
        );

        let result = ledger.handle_transaction_transition(tx_id, wrong_client, TransactionType::Dispute);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not correspond to client"));
    }

    #[test]
    fn handle_transaction_transition_invalid_state() {
        let mut ledger = Ledger::new();
        let tx_id: u32 = 1;
        let client_id: u16 = 1;

        ledger.transactions.insert(
            tx_id,
            TransactionInformation {
                client: client_id,
                amount: Decimal::from_str("50").unwrap(),
                state: TransactionType::Deposit,
            },
        );

        let result = ledger.handle_transaction_transition(tx_id, client_id, TransactionType::Resolve);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid state transition"));
    }

    #[test]
    fn handle_transaction_transition_success() {
        let mut ledger = Ledger::new();
        let tx_id: u32 = 1;
        let client_id: u16 = 1;
        let amount = Decimal::from_str("50").unwrap();

        ledger.transactions.insert(
            tx_id,
            TransactionInformation {
                client: client_id,
                amount,
                state: TransactionType::Deposit,
            },
        );

        let result = ledger.handle_transaction_transition(tx_id, client_id, TransactionType::Dispute);
        assert!(result.is_ok());

        let tx = result.unwrap();
        assert_eq!(tx.amount, amount);
        assert_eq!(tx.state, TransactionType::Dispute);
    }
}
