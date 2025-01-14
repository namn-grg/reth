//! Compact implementation for [`AccessList`]

use crate::{BufMutWritable, Compact};
use alloc::vec::Vec;
use alloy_eips::eip2930::{AccessList, AccessListItem};
use alloy_primitives::{Address, B256};

/// Implement `Compact` for `AccessListItem` and `AccessList`.
impl Compact for AccessListItem {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let mut total_length = 0;
        total_length += self.address.to_compact(buf);
        total_length += self.storage_keys.to_compact(buf);
        total_length
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (address, buf) = Address::from_compact(buf, len);
        let (storage_keys, buf) = Vec::<B256>::from_compact(buf, buf.len());
        (Self { address, storage_keys }, buf)
    }
}

impl Compact for AccessList {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        self.0.to_compact(buf)
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (items, buf) = Vec::<AccessListItem>::from_compact(buf, len);
        (Self(items), buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Bytes;
    use proptest::proptest;
    use proptest_arbitrary_interop::arb;
    use serde::Deserialize;

    proptest! {
        #[test]
        fn test_roundtrip_compact_access_list_item(access_list_item in arb::<AccessListItem>()) {
            let mut compacted_access_list_item = Vec::<u8>::new();
            let len = access_list_item.to_compact(&mut compacted_access_list_item);

            let (decoded_access_list_item, _) = AccessListItem::from_compact(&compacted_access_list_item, len);
            assert_eq!(access_list_item, decoded_access_list_item);
        }
    }

    proptest! {
        #[test]
        fn test_roundtrip_compact_access_list(access_list in arb::<AccessList>()) {
            let mut compacted_access_list = Vec::<u8>::new();
            let len = access_list.to_compact(&mut compacted_access_list);

            let (decoded_access_list, _) = AccessList::from_compact(&compacted_access_list, len);
            assert_eq!(access_list, decoded_access_list);
        }
    }

    #[derive(Deserialize)]
    struct CompactAccessListTestVector {
        access_list: AccessList,
        encoded_bytes: Bytes,
    }

    #[test]
    fn test_compact_access_list_codec() {
        let test_vectors: Vec<CompactAccessListTestVector> =
            serde_json::from_str(include_str!("../../testdata/access_list_compact.json"))
                .expect("Failed to parse test vectors");

        for test_vector in test_vectors {
            let mut buf = Vec::<u8>::new();
            let len = test_vector.access_list.clone().to_compact(&mut buf);
            assert_eq!(test_vector.encoded_bytes.0, buf);

            let (decoded, _) = AccessList::from_compact(&test_vector.encoded_bytes, len);
            assert_eq!(test_vector.access_list, decoded);
        }
    }
}
