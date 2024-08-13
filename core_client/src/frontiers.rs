use crate::{error::CoreClientError, CoreClientConfig};
use log::{debug, error};
use nanopyrs::{block::check_work, rpc::BlockInfo, Account, Block, BlockType, Signature};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

macro_rules! search {
    ($vec: expr, Hash, $value: expr) => {{
        $vec.iter().position(|item| &item.block.hash() == &$value)
    }};

    ($vec: expr, Account, $value: expr) => {{
        $vec.iter().position(|item| &item.block.account == $value)
    }};
}

#[derive(Debug, Clone, Default, Zeroize, Serialize, Deserialize)]
pub struct NewFrontiers {
    pub new: Vec<FrontierInfo>,
}
impl NewFrontiers {
    pub fn merge(mut self, other: NewFrontiers) -> NewFrontiers {
        self.new.extend(other.new);
        self
    }

    pub fn merge_with(&mut self, other: NewFrontiers) {
        self.new.extend(other.new);
    }
}
impl From<Vec<Block>> for NewFrontiers {
    fn from(value: Vec<Block>) -> Self {
        let frontiers: Vec<FrontierInfo> = value
            .into_iter()
            .map(|block| FrontierInfo {
                block,
                cached_work: None,
            })
            .collect();

        NewFrontiers { new: frontiers }
    }
}
impl From<Vec<BlockInfo>> for NewFrontiers {
    fn from(value: Vec<BlockInfo>) -> Self {
        value
            .into_iter()
            .map(|info| info.block.clone())
            .collect::<Vec<Block>>()
            .into()
    }
}
impl From<Vec<FrontierInfo>> for NewFrontiers {
    fn from(value: Vec<FrontierInfo>) -> Self {
        NewFrontiers { new: value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Zeroize, Serialize, Deserialize)]
pub struct FrontierInfo {
    pub block: Block,
    cached_work: Option<[u8; 8]>,
}
impl FrontierInfo {
    pub fn new(block: Block, cached_work: Option<[u8; 8]>) -> FrontierInfo {
        FrontierInfo { block, cached_work }
    }

    pub fn new_unopened(account: Account) -> FrontierInfo {
        let block = Block {
            block_type: BlockType::Change,
            account,
            previous: [0; 32],
            representative: nanopyrs::constants::get_genesis_account(),
            balance: 0,
            link: [0; 32],
            signature: Signature::default(),
            work: [0; 8],
        };
        FrontierInfo {
            block,
            cached_work: None,
        }
    }

    pub fn is_unopened(&self) -> bool {
        FrontierInfo::new_unopened(self.block.account.clone()) == *self
    }

    pub fn cache_work_hash(&self) -> [u8; 32] {
        if self.is_unopened() {
            self.block.account.compressed.to_bytes()
        } else {
            self.block.hash()
        }
    }

    pub fn cached_work(&self) -> Option<[u8; 8]> {
        self.cached_work
    }

    pub fn cache_work(&mut self, config: &CoreClientConfig, work: [u8; 8]) {
        self.cached_work = Some(work);
        if !self.has_valid_work(config) {
            let account = &self.block.account;
            let work_hash = hex::encode_upper(self.cache_work_hash());
            let work = hex::encode(work);
            error!(
                "Attempted to cache invalid work for {account} with work hash {work_hash}: {work}"
            );
            self.clear_work();
        }
    }

    pub fn clear_work(&mut self) {
        self.cached_work = None
    }

    pub fn has_valid_work(&mut self, config: &CoreClientConfig) -> bool {
        if let Some(work) = self.cached_work {
            check_work(
                self.cache_work_hash(),
                config.WORK_DIFFICULTY.to_be_bytes(),
                work,
            )
        } else {
            false
        }
    }
}
impl From<(Block, Option<[u8; 8]>)> for FrontierInfo {
    fn from(value: (Block, Option<[u8; 8]>)) -> Self {
        FrontierInfo {
            block: value.0,
            cached_work: value.1,
        }
    }
}
impl From<FrontierInfo> for (Block, Option<[u8; 8]>) {
    fn from(value: FrontierInfo) -> Self {
        (value.block, value.cached_work)
    }
}

#[derive(Debug, Clone, Default, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct FrontiersDB {
    pub frontiers: Vec<FrontierInfo>,

    /// This is for sanity checking, and **not** necessarily the wallet's balance
    frontiers_balance: u128,
}
impl FrontiersDB {
    /// Returns `None` if the block could not be found
    fn get_hash(&self, hash: [u8; 32]) -> Option<&FrontierInfo> {
        let index = search!(self.frontiers, Hash, hash)?;
        Some(&self.frontiers[index])
    }

    /// Returns `None` if the block could not be found
    fn get_hash_mut(&mut self, hash: [u8; 32]) -> Option<&mut FrontierInfo> {
        let index = search!(self.frontiers, Hash, hash)?;
        Some(&mut self.frontiers[index])
    }

    fn _get_index(&self, index: Option<usize>) -> Option<&FrontierInfo> {
        index.and_then(|index| self.frontiers.get(index))
    }

    fn _get_index_mut(&mut self, index: Option<usize>) -> Option<&mut FrontierInfo> {
        index.and_then(|index| self.frontiers.get_mut(index))
    }

    /// If `Ok()`:
    ///     - If the account already has an entry in the DB, then return `Ok(Some(index))` to replace.
    ///     - If the account does *not* have an entry in the DB, then return `Ok(None)`.
    fn _could_insert(&self, new: &FrontierInfo) -> Result<Option<usize>, CoreClientError> {
        let block = &new.block;
        let mut total = self.frontiers_balance;

        let index = search!(self.frontiers, Account, &new.block.account);

        if let Some(index) = index {
            // if this account already has a DB entry
            let prev = &self.frontiers[index];
            total -= prev.block.balance;

            // epoch blocks sanity check
            if block.block_type.is_epoch() && !block.follows_epoch_rules(&prev.block) {
                return Err(CoreClientError::InvalidEpochBlock);
            }
        }

        // balance sanity check
        if total.checked_add(block.balance).is_none() {
            return Err(CoreClientError::FrontierBalanceOverflow);
        }

        Ok(index)
    }

    /// If `Ok()`:
    ///     - If the account already has an entry in the DB, then return `Ok(Some(index))` to replace.
    ///     - If the account does *not* have an entry in the DB, then return `Ok(None)`.
    fn _could_insert_many(
        &self,
        new: &[FrontierInfo],
    ) -> Result<Vec<Option<usize>>, CoreClientError> {
        new.iter()
            .map(|frontier| self._could_insert(frontier))
            .collect()
    }

    /// Adds a new account and its frontier to the database
    fn _add(&mut self, new: FrontierInfo) {
        assert!(
            self.account_frontier(&new.block.account).is_none(),
            "broken FrontiersDB code: account already exists in the DB"
        );
        self.frontiers_balance += new.block.balance;
        self.frontiers.push(new);
    }

    /// Updates the frontier of an account already in the database
    fn _update(&mut self, index: usize, new: FrontierInfo) -> FrontierInfo {
        assert!(
            self.frontiers[index].block.account == new.block.account,
            "broken FrontiersDB code: index does not match account"
        );
        self.frontiers_balance -= self.frontiers[index].block.balance;
        self.frontiers_balance += new.block.balance;
        std::mem::replace(&mut self.frontiers[index], new)
    }

    /// Adds or updates the frontier of an account which may or may not already be in the database
    fn _add_or_update(&mut self, index: Option<usize>, new: FrontierInfo) {
        match index {
            Some(index) => {
                self._update(index, new);
            }
            None => self._add(new),
        }
    }

    /// Remove an account from the database, and return its frontier
    fn _remove(&mut self, index: usize) -> FrontierInfo {
        self.frontiers_balance -= self.frontiers[index].block.balance;
        self.frontiers.remove(index)
    }

    /// Add or update an account's frontier.
    fn _insert(&mut self, new: FrontierInfo) -> Result<(), CoreClientError> {
        self._add_or_update(self._could_insert(&new)?, new);
        Ok(())
    }

    /// Check whether or not the given downloaded frontiers could be added to the DB
    pub(crate) fn check_new(&self, downloaded: &NewFrontiers) -> Result<(), CoreClientError> {
        self._could_insert_many(&downloaded.new)?;
        Ok(())
    }

    /// Add or update several accounts' frontiers, also handling unopened accounts.
    pub fn insert(&mut self, new: NewFrontiers) -> Result<(), CoreClientError> {
        self._could_insert_many(&new.new)?;
        for info in new.new {
            let index = self._could_insert(&info)?;

            let frontier_hash = hex::encode_upper(info.block.hash());
            let account = &info.block.account;
            let is_new = index.is_none();
            debug!("Adding frontier {frontier_hash} for {account} (new account: {is_new})");

            self._add_or_update(index, info)
        }
        Ok(())
    }

    /// Remove an account from the database, and return whether or not it was in the database
    pub fn remove(&mut self, account: &Account) -> Result<FrontierInfo, CoreClientError> {
        search!(self.frontiers, Account, account)
            .map(|index| self._remove(index))
            .ok_or(CoreClientError::AccountNotFound)
    }

    /// Remove several accounts from the database
    pub fn remove_many(
        &mut self,
        accounts: &[Account],
    ) -> Result<Vec<FrontierInfo>, CoreClientError> {
        accounts
            .iter()
            .map(|account| self.remove(account))
            .collect()
    }

    /// Returns `None` if the account could not be found
    pub fn account_frontier(&self, account: &Account) -> Option<&FrontierInfo> {
        self._get_index(search!(self.frontiers, Account, account))
    }

    /// Returns `None` if an account could not be found
    pub fn accounts_frontiers(&self, accounts: &[Account]) -> Vec<Option<&FrontierInfo>> {
        accounts
            .iter()
            .map(|account| self.account_frontier(account))
            .collect()
    }

    /// Returns all of the accounts in the database
    pub fn all_accounts(&self) -> Vec<Account> {
        self.frontiers
            .iter()
            .map(|frontier| &frontier.block.account)
            .cloned()
            .collect()
    }

    /// Returns `None` if the account could not be found
    pub fn account_frontier_mut(&mut self, account: &Account) -> Option<&mut FrontierInfo> {
        self._get_index_mut(search!(self.frontiers, Account, account))
    }

    /// Returns `None` if the account could not be found
    pub fn account_balance(&self, account: &Account) -> Option<u128> {
        self.account_frontier(account)
            .map(|info| info.block.balance)
    }

    /// Returns `None` if an account could not be found
    pub fn accounts_balances(&self, accounts: &[Account]) -> Vec<Option<u128>> {
        accounts
            .iter()
            .map(|account| self.account_balance(account))
            .collect()
    }

    /// Set the cached work for an account's frontier.
    /// Returns `Err` if the action was not successful.
    pub fn set_account_work(
        &mut self,
        config: &CoreClientConfig,
        account: &Account,
        work: [u8; 8],
    ) -> Result<(), CoreClientError> {
        if let Some(info) = self.account_frontier_mut(account) {
            Ok(info.cache_work(config, work))
        } else {
            Err(CoreClientError::AccountNotFound)
        }
    }

    /// Set the cached work for an account's frontier.
    /// Returns `Err` if the action was not successful.
    pub fn add_work(
        &mut self,
        config: &CoreClientConfig,
        work_hash: [u8; 32],
        work: [u8; 8],
    ) -> Result<(), CoreClientError> {
        if let Some(info) = self.get_hash_mut(work_hash) {
            Ok(info.cache_work(config, work))
        } else {
            Err(CoreClientError::AccountNotFound)
        }
    }

    /// Remove any frontiers which are known to the database.
    /// Useful to avoid re-downloading frontiers.
    pub(crate) fn filter_known_hashes(&self, hashes: &[[u8; 32]]) -> Vec<[u8; 32]> {
        hashes
            .iter()
            .filter(|hash| self.get_hash(**hash).is_none())
            .cloned()
            .collect()
    }

    /// Remove any accounts which are known to the database.
    /// Useful to avoid re-downloading frontiers.
    pub(crate) fn filter_known_accounts(&self, accounts: Vec<Account>) -> Vec<Account> {
        accounts
            .into_iter()
            .filter(|account| self.account_frontier(account).is_none())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanopyrs::Key;

    fn fake_account_1() -> Account {
        "nano_3t6k35gi95xu6tergt6p69ck76ogmitsa8mnijtpxm9fkcm736xtoncuohr3"
            .parse()
            .unwrap()
    }
    fn fake_account_2() -> Account {
        "nano_3qb6o6i1tkzr6jwr5s7eehfxwg9x6eemitdinbpi7u8bjjwsgqfj4wzser3x"
            .parse()
            .unwrap()
    }
    fn fake_account_3() -> Account {
        "nano_3chartsi6ja8ay1qq9xg3xegqnbg1qx76nouw6jedyb8wx3r4wu94rxap7hg"
            .parse()
            .unwrap()
    }

    fn fake_db() -> Result<FrontiersDB, CoreClientError> {
        let mut db = FrontiersDB::default();
        let mut frontiers: NewFrontiers = vec![
            FrontierInfo::new_unopened(fake_account_1()),
            FrontierInfo::new_unopened(fake_account_2()),
            FrontierInfo::new_unopened(fake_account_3()),
        ]
        .into();
        frontiers.new[0].block.balance = 0;
        frontiers.new[1].block.balance = 5;
        frontiers.new[2].block.balance = 10;
        db.insert(frontiers)?;
        Ok(db)
    }

    fn fake_frontier(key: &Key, representative: Account, balance: u128) -> FrontierInfo {
        let mut block = Block {
            block_type: BlockType::Receive,
            account: key.to_account(),
            previous: [0; 32],
            representative,
            balance,
            link: [99; 32],
            signature: Signature::default(),
            work: [0; 8],
        };
        block.sign(key);
        FrontierInfo {
            block,
            cached_work: None,
        }
    }

    #[test]
    fn create() {
        fake_db().unwrap();
    }

    #[test]
    fn accounts() {
        let mut db = fake_db().unwrap();
        assert!(db.all_accounts().contains(&fake_account_1()));
        assert!(db.all_accounts().contains(&fake_account_2()));
        assert!(db.all_accounts().contains(&fake_account_3()));

        assert!(db.account_balance(&fake_account_2()) == Some(5));
        let balances = db.accounts_balances(&vec![fake_account_1(), fake_account_3()]);
        assert!(balances == vec!(Some(0), Some(10)));

        let frontier = db.account_frontier(&fake_account_1()).unwrap();
        assert!(frontier.is_unopened());
        assert!(!frontier.block.has_valid_signature());

        let unknown: Account = "nano_3ktybzzy14zxgb6osbhcc155pwk7osbmf5gbh5fo73bsfu9wuiz54t1uozi1"
            .parse()
            .unwrap();
        let accounts = vec![
            fake_account_2(),
            fake_account_1(),
            unknown.clone(),
            fake_account_3(),
        ];
        assert!(db.filter_known_accounts(accounts) == vec!(unknown));

        db.remove_many(&vec![fake_account_3()]).unwrap();
        assert!(db.all_accounts().contains(&fake_account_1()));
        assert!(db.all_accounts().contains(&fake_account_2()));
        assert!(!db.all_accounts().contains(&fake_account_3()));
    }

    #[test]
    fn insert() {
        let mut db = fake_db().unwrap();
        let key_1 = Key::from_seed(&[9; 32].into(), 9);
        let account_1 = key_1.to_account();
        let frontier_1 = fake_frontier(&key_1, fake_account_2(), 100);
        let key_2 = Key::from_seed(&[10; 32].into(), 10);
        let account_2 = key_2.to_account();
        let frontier_2 = fake_frontier(&key_2, fake_account_3(), 50);
        db.insert(NewFrontiers::from(vec![frontier_1, frontier_2]))
            .unwrap();

        let frontier = db.account_frontier(&account_1).unwrap();
        assert!(frontier.block.has_valid_signature());
        assert!(frontier.block.representative == fake_account_2());
        assert!(db.account_balance(&account_1) == Some(100));

        let frontiers = db.accounts_frontiers(&vec![account_1, account_2.clone()]);
        let frontier = frontiers[1].unwrap();
        assert!(frontier.block.has_valid_signature());
        assert!(frontier.block.representative == fake_account_3());
        assert!(db.account_balance(&account_2) == Some(50));
    }

    #[test]
    fn set_work() {
        let mut config = CoreClientConfig::test_default();
        let mut db = fake_db().unwrap();

        db.set_account_work(&config, &fake_account_1(), [7; 8])
            .unwrap();
        let frontier = db.account_frontier(&fake_account_1()).unwrap();
        assert!(frontier.cached_work == Some([7; 8]));
    }
}
