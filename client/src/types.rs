use super::error::ClientError;
use core_client::constants::ONE_NANO;
use core_client::{nanopyrs::NanoError, Account, CamoAccount, CamoVersion};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop, Serialize, Deserialize)]
pub struct CamoTxSummary {
    pub recipient: CamoAccount,
    pub camo_amount: u128,
    pub total_amount: u128,
    pub notification: [u8; 32],
}
impl Display for CamoTxSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sending {} {} Nano ({} total) with notification {}",
            self.recipient,
            Amount::from(self.camo_amount),
            Amount::from(self.total_amount),
            hex::encode(self.notification)
        )
    }
}

#[derive(Debug, Clone)]
pub enum ParsedAccount {
    Nano(Account),
    Camo(CamoAccount),
}
impl FromStr for ParsedAccount {
    type Err = NanoError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let account = Account::from_str(s).map(ParsedAccount::Nano);
        let camo = CamoAccount::from_str(s).map(ParsedAccount::Camo);
        account.or(camo)
    }
}

#[derive(Debug, Clone)]
pub struct ParsedCamoVersion(pub CamoVersion);
impl FromStr for ParsedCamoVersion {
    type Err = NanoError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let version: u8 = s.parse().map_err(|_| NanoError::IncompatibleCamoVersions)?;
        Ok(ParsedCamoVersion(CamoVersion::try_from(version)?))
    }
}

#[derive(Debug, Clone)]
pub struct Hex32Bytes(pub [u8; 32]);
impl FromStr for Hex32Bytes {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0; 32];
        hex::decode_to_slice(s, &mut bytes)?;
        Ok(Hex32Bytes(bytes))
    }
}
impl From<Hex32Bytes> for [u8; 32] {
    fn from(value: Hex32Bytes) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount {
    pub value: u128,
}
impl From<Amount> for u128 {
    fn from(value: Amount) -> Self {
        value.value
    }
}
impl From<u128> for Amount {
    fn from(value: u128) -> Self {
        Amount { value }
    }
}
impl FromStr for Amount {
    type Err = ClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut amount: Vec<String> = s.split('.').map(|string| string.into()).collect();
        if amount.len() == 1 {
            amount.push('0'.into())
        }

        let amount_0 = amount[0]
            .parse::<u128>()
            .map_err(|_| ClientError::AmountBelowDustThreshold)?
            .checked_mul(ONE_NANO)
            .ok_or(ClientError::AmountBelowDustThreshold)?;
        let amount_1 = format!("{:0<30}", amount[1])
            .parse::<u128>()
            .map_err(|_| ClientError::AmountBelowDustThreshold)?;

        let value = amount_0
            .checked_add(amount_1)
            .ok_or(ClientError::AmountBelowDustThreshold)?;
        Ok(Amount { value })
    }
}
impl Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nano = self.value / ONE_NANO;
        let raw = self.value % ONE_NANO;

        let mut string = format!("{nano}.{raw:0>30}")
            .trim_end_matches('0')
            .to_owned();
        if string.ends_with('.') {
            string.pop();
        }
        write!(f, "{string}")
    }
}

#[cfg(test)]
mod tests {
    use super::Amount;
    use core_client::constants::*;

    fn _amount_from_str(s: &str) -> u128 {
        s.parse::<Amount>().unwrap().value
    }

    #[test]
    fn amount_from_str() {
        assert!(_amount_from_str("1") == ONE_NANO);
        assert!(_amount_from_str("3100") == ONE_NANO * 3100);

        assert!(_amount_from_str("0") == 0);
        assert!(_amount_from_str("0.0") == 0);
        assert!(_amount_from_str("0.01") == ONE_MILLI_NANO * 10);

        assert!(_amount_from_str("1.0") == ONE_NANO);
        assert!(_amount_from_str("21.0") == ONE_NANO * 21);

        assert!(_amount_from_str("0.000000000000000000000000000001") == ONE_RAW);
        assert!(_amount_from_str("0.000000000000000000000000000009") == ONE_RAW * 9);
        assert!(_amount_from_str("0.000000000000000000000000090001") == ONE_RAW * 90001);

        let amount = (ONE_MILLI_NANO * 210_990) + (ONE_RAW * 2);
        assert!(_amount_from_str("210.990000000000000000000000000002") == amount);
        assert!(_amount_from_str("210.990000000000000000000000000002") != amount + 1);
        let amount = (ONE_NANO * 102280) + (ONE_NANO_NANO * 1006);
        assert!(_amount_from_str("102280.000001006") == amount);
        assert!(_amount_from_str("102280.000001006") != amount - 1);
    }

    #[test]
    fn amount_to_str() {
        assert!(Amount::from(0).to_string() == "0");
        assert!(Amount::from(ONE_NANO).to_string() == "1");
        assert!(Amount::from(ONE_NANO * 984302).to_string() == "984302");

        assert!(Amount::from(ONE_MILLI_NANO * 10).to_string() == "0.01");
        assert!(Amount::from(ONE_RAW * 31).to_string() == "0.000000000000000000000000000031");
        assert!(Amount::from(ONE_RAW * 32).to_string() != "0.000000000000000000000000000031");

        let amount = (ONE_NANO * 83) + (ONE_MILLI_NANO * 432);
        assert!(Amount::from(amount).to_string() == "83.432");
        assert!(Amount::from(amount + 1).to_string() != "83.432");
        let amount = (ONE_NANO * 10222) + (ONE_MICRO_NANO * 20022);
        assert!(Amount::from(amount).to_string() == "10222.020022");
        assert!(Amount::from(amount).to_string() != "10222.020023");
    }
}
