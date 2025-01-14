//! Native Compact codec impl for primitive alloy log types.

use crate::{BufMutWritable, Compact};
use alloc::vec::Vec;
use alloy_primitives::{Address, Bytes, Log, LogData, B256};

impl Compact for LogData {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let mut total_length = 0;
        total_length += self.topics().to_compact(buf);
        total_length += self.data.to_compact(buf);
        total_length
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (topics, buf) = Vec::<B256>::from_compact(buf, len);
        let (data, buf) = Bytes::from_compact(buf, buf.len());
        (Self::new_unchecked(topics, data), buf)
    }
}

impl Compact for Log {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let mut total_length = 0;
        total_length += self.address.to_compact(buf);
        total_length += self.data.to_compact(buf);
        total_length
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (address, buf) = Address::from_compact(buf, len);
        let (data, buf) = LogData::from_compact(buf, buf.len());
        (Self { address, data }, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::proptest;

    proptest! {
        #[test]
        fn roundtrip(log: Log) {
            let mut buf = crate::WritableBuffer::new();
            let len = log.to_compact(&mut buf);
            let (decoded, _) = Log::from_compact(buf.written_slice(), len);
            assert_eq!(log, decoded);
        }
    }

    #[test]
    fn test_vec_log_roundtrip() {
        let logs = vec![
            Log {
                address: Address::random(),
                data: LogData::new_unchecked(vec![B256::random()], Bytes::from(vec![1, 2, 3])),
            },
            Log {
                address: Address::random(),
                data: LogData::new_unchecked(
                    vec![B256::random(), B256::random()],
                    Bytes::from(vec![4, 5, 6]),
                ),
            },
        ];

        let mut buf = crate::WritableBuffer::new();
        let len = logs.to_compact(&mut buf);
        let (decoded, _) = Vec::<Log>::from_compact(buf.written_slice(), len);
        assert_eq!(logs, decoded);
    }
}
