use super::config::ClientConfig;
use super::error::ClientError;
use log::debug;
use nanopyrs::{camo::*, Account, Block, Key, SecretBytes};
use serde::{Deserialize, Serialize};
use std::convert::From;
use std::fmt::Display;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, PartialEq, Eq, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct GenericInfo<T: Clone + Eq + Zeroize> {
    /// Index of the Account
    pub index: u32,
    pub account: T,
}
pub type AccountInfo = GenericInfo<Account>;
pub type CamoAccountInfo = GenericInfo<CamoAccount>;

#[derive(Clone, Debug, PartialEq, Eq, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct DerivedAccountInfo {
    /// Camo version used to derive this account
    pub versions: CamoVersions,
    /// ECDH secret
    pub secret: SecretBytes<32>,
    /// Index of the master Camo Account
    pub master_index: u32,
    /// Index on the shared seed (currently always 0)
    pub index: u32,
    pub account: Account,
}

macro_rules! _search_db {
    ($db: expr, Notification, $account: expr, $iter: ident) => {{
        $db.info
            .$iter()
            .find(|item| &item.account.signer_account() == $account)
    }};

    ($db: expr, CamoAccount, $account: expr, $iter: ident) => {{
        _search_db!($db, Account, $account, $iter)
    }};

    ($db: expr, Account, $value: expr, $iter: ident) => {{
        $db.info.$iter().find(|item| item.account == $value)
    }};

    ($db: expr, Index, $value: expr, $iter: ident) => {{
        $db.info.$iter().find(|item| item.index == $value)
    }};
}

macro_rules! search_db {
    (mut $db: expr, Remove, $value: expr) => {{
        $db.info
            .iter()
            .position(|info| &info.account == $value)
            .ok_or(ClientError::AccountNotFound)
            .map(|index| $db.info.remove(index))
    }};

    ($db: expr, $type: ident, $account: expr) => {{
        _search_db!($db, $type, $account, iter)
    }};
    (mut $db: expr, $type: ident, $account: expr) => {{
        _search_db!($db, $type, $account, iter_mut)
    }};
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct GenericInfoDB<T: Clone + Eq + Zeroize + Display> {
    /// **Unordered!** The index of accounts in this does not necessarily represent their actual wallet index
    pub info: Vec<GenericInfo<T>>,
}
impl<T: Clone + Eq + Zeroize + Display> GenericInfoDB<T> {
    pub fn new() -> GenericInfoDB<T> {
        Self::default()
    }

    pub fn all_infos(&self) -> &[GenericInfo<T>] {
        &self.info
    }

    pub fn all_accounts(&self) -> Vec<T> {
        self.info.iter().map(|info| info.account.clone()).collect()
    }

    /// Insert an account to the DB, regardless of whether or not the account limit has been reached.
    ///
    /// Returns whether or not the DB already contained the account.
    pub fn force_insert(&mut self, info: GenericInfo<T>) -> bool {
        if self.contains(&info.account) {
            return true;
        }
        debug!("Adding {} to wallet DB", info.account);
        self.info.push(info);
        false
    }

    /// Insert an account to the DB.
    ///
    /// Returns `Err` if the limit has been reached on how many accounts can be tracked at one time.
    /// See the `DB_NUMBER_OF_ACCOUNTS_LIMIT` configuration option.
    ///
    /// Otherwise, returns whether or not the DB already contained the account.
    pub fn insert(
        &mut self,
        config: &ClientConfig,
        info: GenericInfo<T>,
    ) -> Result<bool, ClientError> {
        if self.info.len() >= config.DB_NUMBER_OF_ACCOUNTS_LIMIT {
            return Err(ClientError::DBAccountLimitReached);
        }
        Ok(self.force_insert(info))
    }

    /// Remove an account from the DB, returning the account info if successful.
    pub fn remove(&mut self, account: &T) -> Result<GenericInfo<T>, ClientError> {
        search_db!(mut self, Remove, account)
    }

    pub fn get_info(&self, account: &T) -> Option<&GenericInfo<T>> {
        search_db!(self, Account, *account)
    }

    pub fn get_info_from_index(&self, index: u32) -> Option<&GenericInfo<T>> {
        search_db!(self, Index, index)
    }

    pub fn get_mut_info(&mut self, account: &T) -> Option<&mut GenericInfo<T>> {
        search_db!(mut self, Account, *account)
    }

    pub fn get_mut_info_from_index(&mut self, index: u32) -> Option<&mut GenericInfo<T>> {
        search_db!(mut self, Index, index)
    }

    pub fn contains(&self, account: &T) -> bool {
        self.get_info(account).is_some()
    }

    pub fn contains_index(&self, index: u32) -> bool {
        self.get_info_from_index(index).is_some()
    }
}
impl GenericInfoDB<CamoAccount> {
    pub fn all_notification_accounts(&self) -> Vec<Account> {
        self.info
            .iter()
            .map(|info| info.account.signer_account())
            .collect()
    }

    pub fn get_info_from_notification_account(
        &self,
        account: &Account,
    ) -> Option<&GenericInfo<CamoAccount>> {
        search_db!(self, Notification, account)
    }

    pub fn contains_notification_account(&self, account: &Account) -> bool {
        self.get_info_from_notification_account(account).is_some()
    }
}
impl<T: Clone + Eq + Zeroize + Display> Default for GenericInfoDB<T> {
    fn default() -> Self {
        GenericInfoDB { info: vec![] }
    }
}

pub type AccountDB = GenericInfoDB<Account>;
pub type CamoAccountDB = GenericInfoDB<CamoAccount>;

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Default, Serialize, Deserialize)]
pub struct DerivedAccountDB {
    /// **Unordered!** The index of accounts in this does not necessarily represent their actual wallet index
    pub info: Vec<DerivedAccountInfo>,
}

impl DerivedAccountDB {
    pub fn new() -> DerivedAccountDB {
        Self::default()
    }

    pub fn all_infos(&self) -> &[DerivedAccountInfo] {
        &self.info
    }

    pub fn all_accounts(&self) -> Vec<Account> {
        self.info.iter().map(|info| info.account.clone()).collect()
    }

    /// Insert an account to the DB.
    ///
    /// Returns whether or not the DB already contained the account
    pub fn insert(&mut self, info: DerivedAccountInfo) -> bool {
        if self.contains(&info.account) {
            return true;
        }
        debug!("Adding {} to wallet DB", info.account);
        self.info.push(info);
        false
    }

    /// Insert many accounts to the DB.
    pub fn insert_many(&mut self, infos: Vec<DerivedAccountInfo>) {
        for info in infos {
            self.insert(info);
        }
    }

    /// Remove an account from the DB, returning the account info if successful.
    pub fn remove(&mut self, account: &Account) -> Result<DerivedAccountInfo, ClientError> {
        search_db!(mut self, Remove, account)
    }

    pub fn get_info(&self, account: &Account) -> Option<&DerivedAccountInfo> {
        search_db!(self, Account, *account)
    }

    pub fn get_info_from_index(&self, index: u32) -> Option<&DerivedAccountInfo> {
        search_db!(self, Index, index)
    }

    pub fn get_info_from_master(
        &self,
        camo_account_db: &CamoAccountDB,
        master: &CamoAccount,
    ) -> Vec<&DerivedAccountInfo> {
        let index = match camo_account_db.get_info(master) {
            Some(info) => info.index,
            None => return vec![],
        };

        self.info
            .iter()
            .filter(|item| item.master_index == index)
            .collect::<Vec<&DerivedAccountInfo>>()
    }

    pub fn get_mut_info(&mut self, account: &Account) -> Option<&mut DerivedAccountInfo> {
        search_db!(mut self, Account, *account)
    }

    pub fn get_mut_info_from_index(&mut self, index: u32) -> Option<&mut DerivedAccountInfo> {
        search_db!(mut self, Index, index)
    }

    pub fn contains(&self, account: &Account) -> bool {
        self.get_info(account).is_some()
    }

    pub fn contains_index(&self, index: u32) -> bool {
        self.get_info_from_index(index).is_some()
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct WalletSeed {
    bytes: SecretBytes<32>,
}
impl WalletSeed {
    pub fn from_seed_hex(mut seed: String) -> Result<WalletSeed, ClientError> {
        let seed_bytes: [u8; 32] = hex::decode(&seed)
            .map_err(|_| ClientError::InvalidSeed)?
            .try_into()
            .or(Err(ClientError::InvalidSeed))?;
        let result = Ok(WalletSeed::from(seed_bytes));
        seed.zeroize();
        result
    }

    pub fn as_hex(&self) -> String {
        hex::encode(self.bytes.as_ref())
    }

    pub fn get_key(&self, index: u32) -> (Key, AccountInfo) {
        let key = Key::from_seed(&self.bytes, index);
        let account = key.to_account();
        let info = AccountInfo { account, index };
        (key, info)
    }

    /// Returns `Some((key, account_info))`, or `None` if no supported version is given
    pub fn get_camo_key(
        &self,
        index: u32,
        versions: CamoVersions,
    ) -> Option<(CamoKeys, CamoAccountInfo)> {
        let key = CamoKeys::from_seed(&self.bytes, index, versions)?;
        let account = key.to_camo_account();
        let info = CamoAccountInfo { account, index };
        Some((key, info))
    }

    pub fn derive_key_from_secret(
        &self,
        master: &CamoAccountInfo,
        secret: SecretBytes<32>,
    ) -> (Key, DerivedAccountInfo) {
        let (master_key, _) = self
            .get_camo_key(master.index, master.account.camo_versions())
            .expect("broken derive_key_from_secret code: invalid camo key");
        let key = master_key.derive_key(&secret);
        let info = DerivedAccountInfo {
            versions: master_key.camo_versions(),
            secret,
            master_index: master.index,
            index: 0,
            account: key.to_account(),
        };
        (key, info)
    }

    pub fn derive_key(
        &self,
        master: &CamoAccountInfo,
        notification: &Notification,
    ) -> (Key, DerivedAccountInfo) {
        let (master_key, _) = self
            .get_camo_key(master.index, master.account.camo_versions())
            .expect("broken derive_key_from_secret code: invalid camo key");
        self.derive_key_from_secret(master, master_key.receiver_ecdh(notification))
    }
}
impl From<[u8; 32]> for WalletSeed {
    fn from(bytes: [u8; 32]) -> Self {
        WalletSeed::from(SecretBytes::from(bytes))
    }
}
impl From<SecretBytes<32>> for WalletSeed {
    fn from(bytes: SecretBytes<32>) -> Self {
        WalletSeed { bytes }
    }
}

#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop, Default, Serialize, Deserialize)]
pub struct WalletDB {
    pub account_db: AccountDB,
    pub camo_account_db: CamoAccountDB,
    pub derived_account_db: DerivedAccountDB,
}
impl WalletDB {
    /// Find the key of the given `nano_` account in this wallet, regardless of where it is located.
    /// Returns `None` if the account could not be found.
    pub fn find_key(&self, seed: &WalletSeed, account: &Account) -> Option<Key> {
        let key = self
            .get_key_from_account(seed, account)
            .or(self.get_derived_key_from_account(seed, account))
            .or(self.get_camo_notification_key(seed, account))?;
        assert!(
            &key.to_account() == account,
            "broken find_key code: wrong key"
        );
        Some(key)
    }
    fn get_key_from_account(&self, seed: &WalletSeed, account: &Account) -> Option<Key> {
        let index = self.account_db.get_info(account)?.index;
        Some(Key::from_seed(&seed.bytes, index))
    }
    fn get_derived_key_from_account(&self, seed: &WalletSeed, account: &Account) -> Option<Key> {
        let info = self.derived_account_db.get_info(account)?;
        let master = CamoKeys::from_seed(&seed.bytes, info.master_index, info.versions)?;
        Some(master.derive_key(&info.secret))
    }
    fn get_camo_notification_key(&self, seed: &WalletSeed, account: &Account) -> Option<Key> {
        let master = self
            .camo_account_db
            .get_info_from_notification_account(account)?;
        let master = self.find_camo_key(seed, &master.account)?;
        Some(master.signer_key())
    }

    /// Find the key of the given `camo_` account in this wallet.
    /// Returns `None` if the account could not be found.
    pub fn find_camo_key(&self, seed: &WalletSeed, account: &CamoAccount) -> Option<CamoKeys> {
        let index = self.camo_account_db.get_info(account)?.index;
        let key = CamoKeys::from_seed(&seed.bytes, index, account.camo_versions())
            .expect("broken Wallet code");
        Some(key)
    }

    /// Find the key of the given notification's `camo_` account in this wallet.
    /// Returns `None` if the account could not be found.
    pub fn find_camo_key_from_notification_account(
        &self,
        seed: &WalletSeed,
        account: &Account,
    ) -> Option<CamoKeys> {
        let account_info = self
            .camo_account_db
            .get_info_from_notification_account(account)?;
        let key = CamoKeys::from_seed(
            &seed.bytes,
            account_info.index,
            account_info.account.camo_versions(),
        )
        .expect("broken Wallet code");
        Some(key)
    }

    /// Returns whether or not the key of the given `nano_` account is known, regardless of where it is located.
    pub fn contains_account(&self, account: &Account) -> bool {
        self.account_db.get_info(account).is_some()
            || self
                .camo_account_db
                .get_info_from_notification_account(account)
                .is_some()
            || self.derived_account_db.get_info(account).is_some()
    }

    /// Returns whether or not the key of the given `camo_` account is known.
    pub fn contains_camo_account(&self, account: &CamoAccount) -> bool {
        self.camo_account_db.get_info(account).is_some()
    }

    /// Returns all on-chain accounts controlled by this wallet, except for derived accounts
    pub fn public_nano_accounts(&self) -> Vec<Account> {
        [
            self.account_db.all_accounts(),
            self.camo_account_db.all_notification_accounts(),
        ]
        .concat()
    }

    /// Returns all on-chain accounts controlled by this wallet, regardless of where they are located
    pub fn all_nano_accounts(&self) -> Vec<Account> {
        [
            self.account_db.all_accounts(),
            self.camo_account_db.all_notification_accounts(),
            self.derived_account_db.all_accounts(),
        ]
        .concat()
    }

    /// sign the given block, returning it with a signature attached
    pub fn sign_block(&self, seed: &WalletSeed, mut block: Block) -> Result<Block, ClientError> {
        let key = self
            .find_key(seed, &block.account)
            .ok_or(ClientError::AccountNotFound)?;
        block.sign(&key);
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanopyrs::{camo::CamoAccount, BlockType, Signature};

    fn fake_key() -> Key {
        Key::from_seed(&[99; 32].into(), 9999)
    }
    fn fake_notification(camo: &CamoAccount) -> Notification {
        camo.sender_ecdh(&fake_key(), [29; 32]).1
    }
    fn fake_account() -> Account {
        "nano_3i1aq1cchnmbn9x5rsbap8b15akfh7wj7pwskuzi7ahz8oq6cobd99d4r3b7"
            .parse()
            .unwrap()
    }
    fn fake_camo_account() -> CamoAccount {
        "camo_18wydi3gmaw4aefwhkijrjw4qd87i4tc85wbnij95gz4em3qssickhpoj9i4t6taqk46wdnie7aj8ijrjhtcdgsp3c1oqnahct3otygxx4k7f3o4".parse().unwrap()
    }
    fn fake_seed() -> Result<WalletSeed, ClientError> {
        let seed_hex = "c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8";
        WalletSeed::from_seed_hex(seed_hex.into())
    }
    fn fake_db() -> Result<WalletDB, ClientError> {
        let seed = fake_seed()?;
        let mut db = WalletDB::default();

        let (_, info) = seed.get_key(91);
        db.account_db.insert(&ClientConfig::test_default(), info)?;
        let (_, info) = seed.get_key(92);
        db.account_db.insert(&ClientConfig::test_default(), info)?;
        let (camo_key, camo_info) = seed.get_camo_key(99, camo_versions()).unwrap();
        db.camo_account_db
            .insert(&ClientConfig::test_default(), camo_info.clone())?;
        let camo_account = camo_key.to_camo_account();

        let (sender_ecdh, notification) = camo_account.sender_ecdh(&fake_key(), [29; 32]);
        let derived_account = camo_account.derive_account(&sender_ecdh);
        let (derived, info) = seed.derive_key(&camo_info, &notification);
        assert!(derived_account == derived.to_account());

        db.derived_account_db.insert(info);
        Ok(db)
    }
    fn camo_versions() -> CamoVersions {
        CamoVersions::decode_from_bits(0x01)
    }

    #[test]
    fn seed_from_hex() {
        let seed_hex: String =
            "d9c8c8c8c8c8c8c8c8c8c8c8c8eac8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8c8b7".into();
        let seed = WalletSeed::from_seed_hex(seed_hex.clone()).unwrap();
        assert!(seed.as_hex() == seed_hex);
    }

    #[test]
    fn seed_get_account() {
        let account = WalletSeed::from([0; 32]).get_key(0).0.to_account();
        assert!(account == fake_account());
    }

    #[test]
    fn seed_get_camo_account() {
        let account = WalletSeed::from([0; 32])
            .get_camo_key(0, camo_versions())
            .unwrap()
            .0
            .to_camo_account();
        assert!(account == fake_camo_account());
    }

    #[test]
    fn db_insert() {
        let seed = fake_seed().unwrap();
        let db = fake_db().unwrap();

        assert!(db.all_nano_accounts().len() == 4);
        assert!(db.public_nano_accounts().len() == 3);

        let account = seed.get_key(91).0.to_account();
        assert!(db.all_nano_accounts().contains(&account));
        let account = seed.get_key(92).0.to_account();
        assert!(db.all_nano_accounts().contains(&account));
        let account = seed
            .get_camo_key(99, camo_versions())
            .unwrap()
            .0
            .to_camo_account()
            .signer_account();
        assert!(db.all_nano_accounts().contains(&account));
    }

    #[test]
    fn db_find_key() {
        let seed = fake_seed().unwrap();
        let db = fake_db().unwrap();
        let (key, _) = seed.get_key(91);
        let account = key.to_account();

        assert!(db.contains_account(&account));
        db.find_key(&seed, &account).unwrap();
    }

    #[test]
    fn db_find_camo_key() {
        let seed = fake_seed().unwrap();
        let db = fake_db().unwrap();
        let (key, _) = seed.get_camo_key(99, camo_versions()).unwrap();
        let account = key.to_camo_account();

        assert!(db.contains_camo_account(&account));
        db.find_camo_key(&seed, &account).unwrap();
    }

    #[test]
    fn db_find_derived_key() {
        let seed = fake_seed().unwrap();
        let db = fake_db().unwrap();
        let (camo_key, camo_info) = seed.get_camo_key(99, camo_versions()).unwrap();

        let notification = fake_notification(&camo_key.to_camo_account());
        let (key, _) = seed.derive_key(&camo_info, &notification);
        let account = key.to_account();

        assert!(db.contains_account(&account));
        db.find_key(&seed, &account).unwrap();
    }

    #[test]
    fn db_sign_block() {
        let seed = fake_seed().unwrap();
        let key_1 = seed.get_key(91).0;
        let db = fake_db().unwrap();

        let mut block = Block {
            block_type: BlockType::Receive,
            account: key_1.to_account(),
            previous: [22; 32],
            representative: fake_account(),
            balance: 999,
            link: [201; 32],
            signature: Signature::default(),
            work: [0; 8],
        };
        block = db.sign_block(&seed, block).unwrap();
        assert!(block.has_valid_signature())
    }
}
