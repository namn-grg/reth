use reth_codecs::{add_arbitrary_tests, Compact};
use serde::Serialize;

use alloy_primitives::{bytes::Buf, Address};
use reth_primitives_traits::Account;

/// Account as it is saved in the database.
///
/// [`Address`] is the subkey.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
#[cfg_attr(any(test, feature = "arbitrary"), derive(arbitrary::Arbitrary, serde::Deserialize))]
#[add_arbitrary_tests(compact)]
pub struct AccountBeforeTx {
    /// Address for the account. Acts as `DupSort::SubKey`.
    pub address: Address,
    /// Account state before the transaction.
    pub info: Option<Account>,
}

// NOTE: Removing reth_codec and manually encode subkey
// and compress second part of the value. If we have compression
// over whole value (Even SubKey) that would mess up fetching of values with seek_by_key_subkey
impl Compact for AccountBeforeTx {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let mut acc_len = 0;
        if let Some(account) = &self.info {
            acc_len = account.to_compact(buf);
        }
        acc_len
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        if len == 0 {
            return (Self { info: None }, buf);
        }
        let (info, buf) = Account::from_compact(buf, len);
        (Self { info: Some(info) }, buf)
    }
}
