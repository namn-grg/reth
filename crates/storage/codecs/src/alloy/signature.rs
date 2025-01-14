//! Compact implementation for [`Signature`]

use crate::{BufMutWritable, Compact};
use alloy_primitives::{PrimitiveSignature as Signature, U256};
use bytes::Buf;

impl Compact for Signature {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        buf.put_slice(&self.r().as_le_bytes());
        buf.put_slice(&self.s().as_le_bytes());
        64
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        assert!(buf.len() >= 64);
        let r = U256::from_le_slice(&buf[0..32]);
        let s = U256::from_le_slice(&buf[32..64]);
        (Self::new(r, s, len != 0), &buf[64..])
    }
}
