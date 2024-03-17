use crate::error::ClientError;
use nanopyrs::rpc::RpcError;
use std::fmt::Debug;

#[derive(Debug)]
pub struct RpcFailure {
    pub err: RpcError,
    pub url: String,
}

#[derive(Debug, Default)]
pub struct RpcFailures(pub Vec<RpcFailure>);
impl RpcFailures {
    pub fn merge(mut self, other: RpcFailures) -> RpcFailures {
        self.0.extend(other.0);
        self
    }

    pub fn merge_with(&mut self, other: RpcFailures) {
        self.0.extend(other.0);
    }

    pub fn merge_all(failures: Vec<RpcFailures>) -> RpcFailures {
        let failures: Vec<RpcFailure> = failures
            .into_iter()
            .flat_map(|failures| failures.0)
            .collect();
        RpcFailures(failures)
    }
}

#[derive(Debug, Default)]
pub struct RpcSuccess<T> {
    pub item: T,
    pub failures: RpcFailures,
}
impl<T> From<RpcSuccess<T>> for (T, RpcFailures) {
    fn from(value: RpcSuccess<T>) -> Self {
        (value.item, value.failures)
    }
}
impl<T> From<(T, RpcFailures)> for RpcSuccess<T> {
    fn from(value: (T, RpcFailures)) -> Self {
        RpcSuccess {
            item: value.0,
            failures: value.1,
        }
    }
}

pub type RpcResult<T> = Result<RpcSuccess<T>, ClientError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge() {
        let failure_1 = RpcFailures(vec![RpcFailure {
            err: RpcError::InvalidData,
            url: "https://example.com".into(),
        }]);
        let failure_2 = RpcFailures(vec![RpcFailure {
            err: RpcError::InvalidAccount,
            url: "https://example2.com".into(),
        }]);
        let failures = RpcFailures::merge_all(vec![failure_1, failure_2]);

        assert!(failures.0.len() == 2);
        assert!(&failures.0[0].url == "https://example.com");
        assert!(&failures.0[1].url == "https://example2.com");
    }
}
