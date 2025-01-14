//! Compact codec.
//!
//! *Warning*: The `Compact` encoding format and its implementations are
//! designed for storing and retrieving data internally. They are not hardened
//! to safely read potentially malicious data.
//!
//! ## Feature Flags
//!
//! - `alloy`: [Compact] implementation for various alloy types.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use reth_codecs_derive::*;
use serde as _;

use alloy_primitives::{Address, Bloom, Bytes as AlloyBytes, FixedBytes, U256};
use bytes::{self, Buf, BufMut};
use core::mem::MaybeUninit;

use alloc::{
    borrow::{Cow, ToOwned},
    string::String,
    vec::Vec,
};

#[cfg(feature = "test-utils")]
pub mod alloy;

#[cfg(not(feature = "test-utils"))]
#[cfg(any(test, feature = "alloy"))]
mod alloy;

pub mod txtype;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// Used by generated code and doc tests. Not public API.
#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;

mod buf_mut_writable;

pub use buf_mut_writable::{BufMutWritable, WritableBuffer};

/// A trait for types that can be encoded and decoded in a compact format.
pub trait Compact: Sized {
    /// Encodes the type into a buffer and returns the number of bytes written.
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable;

    /// Decodes the type from a buffer and returns the decoded type along with the remaining buffer.
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]);

    /// Optional specialized encoding for fixed-size types.
    /// If there's no good reason to use it, don't.
    #[inline]
    fn specialized_to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        self.to_compact(buf)
    }

    /// Optional specialized decoding for fixed-size types.
    /// If there's no good reason to use it, don't.
    #[inline]
    fn specialized_from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        Self::from_compact(buf, len)
    }
}

impl Compact for alloc::string::String {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        self.as_bytes().to_compact(buf)
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (vec, buf) = Vec::<u8>::from_compact(buf, len);
        let string = Self::from_utf8(vec).unwrap(); // Safe conversion
        (string, buf)
    }
}

impl<T: Compact> Compact for &T {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        (*self).to_compact(buf)
    }

    fn from_compact(_: &[u8], _: usize) -> (Self, &[u8]) {
        unimplemented!()
    }
}

/// To be used with `Option<CompactPlaceholder>` to place or replace one bit on the bitflag struct.
pub type CompactPlaceholder = ();

impl Compact for CompactPlaceholder {
    #[inline]
    fn to_compact<B>(&self, _: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        0
    }

    #[inline]
    fn from_compact(buf: &[u8], _: usize) -> (Self, &[u8]) {
        ((), buf)
    }
}

macro_rules! impl_uint_compact {
    ($($name:tt),+) => {
        $(
            impl Compact for $name {
                #[inline]
                fn to_compact<B>(&self, buf: &mut B) -> usize
                    where B: BufMutWritable
                {
                    let leading = self.leading_zeros() as usize / 8;
                    buf.put_slice(&self.to_be_bytes()[leading..]);
                    core::mem::size_of::<$name>() - leading
                }

                #[inline]
                fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
                    if len == 0 {
                        return (0, buf);
                    }

                    let mut arr = [0; core::mem::size_of::<$name>()];
                    arr[core::mem::size_of::<$name>() - len..].copy_from_slice(&buf[..len]);
                    buf.advance(len);
                    ($name::from_be_bytes(arr), buf)
                }
            }
        )+
    };
}

impl_uint_compact!(u8, u64, u128);

impl<T> Compact for Vec<T>
where
    T: Compact,
{
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        encode_varuint(self.len(), buf);
        let mut tmp = WritableBuffer::with_capacity(64);

        for element in self {
            tmp.clear();
            let length = element.to_compact(&mut tmp);
            encode_varuint(length, buf);
            buf.put_slice(tmp.written_slice());
        }

        0
    }

    #[inline]
    fn from_compact(buf: &[u8], _: usize) -> (Self, &[u8]) {
        let (length, mut buf) = decode_varuint(buf);
        let mut list = Self::with_capacity(length);
        for _ in 0..length {
            let len;
            (len, buf) = decode_varuint(buf);

            let (element, _) = T::from_compact(&buf[..len], len);
            buf.advance(len);

            list.push(element);
        }

        (list, buf)
    }
}

impl<T> Compact for &[T]
where
    T: Compact,
{
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        encode_varuint(self.len(), buf);
        let mut tmp = WritableBuffer::with_capacity(64);

        for element in *self {
            tmp.clear();
            let length = element.to_compact(&mut tmp);
            encode_varuint(length, buf);
            buf.put_slice(tmp.written_slice());
        }

        0
    }

    #[inline]
    fn from_compact(_: &[u8], _: usize) -> (Self, &[u8]) {
        unimplemented!()
    }
}

impl<T> Compact for Option<T>
where
    T: Compact,
{
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let Some(element) = self else { return 0 };

        let mut tmp = WritableBuffer::with_capacity(64);
        let length = element.to_compact(&mut tmp);
        encode_varuint(length, buf);
        buf.put_slice(tmp.written_slice());

        1
    }

    #[inline]
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        if len == 0 {
            return (None, buf)
        }

        let (len, mut buf) = decode_varuint(buf);
        let (element, _) = T::from_compact(&buf[..len], len);
        buf.advance(len);

        (Some(element), buf)
    }
}

impl<T: Compact + ToOwned<Owned = T>> Compact for Cow<'_, T> {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        self.as_ref().to_compact(buf)
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (element, buf) = T::from_compact(buf, len);
        (Cow::Owned(element), buf)
    }
}

impl Compact for U256 {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let inner = self.to_be_bytes::<32>();
        let size = 32 - (self.leading_zeros() / 8);
        buf.put_slice(&inner[32 - size..]);
        size
    }

    #[inline]
    fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
        if len == 0 {
            return (Self::ZERO, buf)
        }

        let mut arr = [0; 32];
        arr[(32 - len)..].copy_from_slice(&buf[..len]);
        buf.advance(len);
        (Self::from_be_bytes(arr), buf)
    }
}

impl Compact for AlloyBytes {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let len = self.len();
        buf.put_slice(self.as_ref());
        len
    }

    #[inline]
    fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
        let bytes = buf[..len].to_vec();
        buf.advance(len);
        (bytes.into(), buf)
    }
}

impl<const N: usize> Compact for [u8; N] {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        buf.put_slice(&self[..]);
        N
    }

    #[inline]
    fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
        if len == 0 {
            return ([0; N], buf)
        }

        let v = buf[..N].try_into().unwrap();
        buf.advance(N);
        (v, buf)
    }
}

/// Implements the [`Compact`] trait for wrappers over fixed size byte array types.
#[macro_export]
macro_rules! impl_compact_for_wrapped_bytes {
    ($($name:tt),+) => {
        $(
            impl Compact for $name {
                #[inline]
                fn to_compact<B>(&self, buf: &mut B) -> usize
                where
                    B: BufMutWritable
                {
                    self.0.to_compact(buf)
                }

                #[inline]
                fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
                    let (v, buf) = <[u8; core::mem::size_of::<$name>()]>::from_compact(buf, len);
                    (Self::from(v), buf)
                }
            }
        )+
    };
}
impl_compact_for_wrapped_bytes!(Address, Bloom);

impl<const N: usize> Compact for FixedBytes<N> {
    #[inline]
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        self.0.to_compact(buf)
    }

    #[inline]
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (v, buf) = <[u8; N]>::from_compact(buf, len);
        (Self::from(v), buf)
    }
}

impl Compact for bool {
    /// `bool` vars go directly to the `StructFlags` and are not written to the buffer.
    #[inline]
    fn to_compact<B>(&self, _: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        *self as usize
    }

    /// `bool` expects the real value to come in `len`, and does not advance the cursor.
    #[inline]
    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        (len != 0, buf)
    }
}

fn encode_varuint<B>(mut n: usize, buf: &mut B)
where
    B: BufMutWritable,
{
    while n >= 0x80 {
        buf.put_u8((n as u8) | 0x80);
        n >>= 7;
    }
    buf.put_u8(n as u8);
}

fn decode_varuint(mut buf: &[u8]) -> (usize, &[u8]) {
    let mut n = 0;
    let mut shift = 0;

    loop {
        let byte = buf[0];
        buf.advance(1);

        n |= ((byte & 0x7f) as usize) << shift;

        if byte & 0x80 == 0 {
            break
        }

        shift += 7;
    }

    (n, buf)
}

#[inline(never)]
#[cold]
const fn decode_varuint_panic() -> ! {
    panic!("could not decode varuint");
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use serde::{Deserialize, Serialize};

    #[test]
    fn test_compact_bytes() {
        let arr = [1, 2, 3, 4, 5];
        let list = AlloyBytes::copy_from_slice(&arr);
        let mut buf = WritableBuffer::with_capacity(list.len() + 1);
        assert_eq!(list.to_compact(&mut buf), list.len());

        buf.put_u8(1);

        assert_eq!(&buf.written_slice()[..arr.len()], &arr);
        assert_eq!(
            AlloyBytes::from_compact(buf.written_slice(), list.len()),
            (list, vec![1].as_slice())
        );
    }

    #[test]
    fn test_compact_address() {
        let mut buf = WritableBuffer::with_capacity(21);
        assert_eq!(Address::ZERO.to_compact(&mut buf), 20);
        assert_eq!(buf.written_slice(), &[0; 20]);

        // Add some noise data
        buf.put_u8(1);

        // Address shouldn't care about the len passed, since it's not actually compacted
        assert_eq!(
            Address::from_compact(buf.written_slice(), 1000),
            (Address::ZERO, vec![1u8].as_slice())
        );
    }

    #[test]
    fn test_compact_b256() {
        let mut buf = WritableBuffer::with_capacity(33);
        assert_eq!(B256::ZERO.to_compact(&mut buf), 32);
        assert_eq!(buf.written_slice(), &[0; 32]);

        // Add some noise data
        buf.put_u8(1);

        // B256 shouldn't care about the len passed, since it's not actually compacted
        assert_eq!(
            B256::from_compact(buf.written_slice(), 1000),
            (B256::ZERO, vec![1u8].as_slice())
        );
    }

    #[test]
    fn test_compact_bool() {
        let mut buf = WritableBuffer::new();

        assert_eq!(true.to_compact(&mut buf), 1);
        // Bool vars go directly to the `StructFlags` and not written to the buf
        assert_eq!(buf.written_len(), 0);

        assert_eq!(false.to_compact(&mut buf), 0);
        assert_eq!(buf.written_len(), 0);

        // Create a buffer with some test data
        let mut buf = WritableBuffer::new();
        buf.put_u8(100);

        // Bool expects the real value to come in `len`, and does not advance the cursor
        assert_eq!(bool::from_compact(buf.written_slice(), 1), (true, buf.written_slice()));
        assert_eq!(bool::from_compact(buf.written_slice(), 0), (false, buf.written_slice()));
    }

    #[test]
    fn test_compact_option() {
        let opt = Some(B256::ZERO);
        let mut buf = WritableBuffer::with_capacity(33);

        assert_eq!(None::<B256>.to_compact(&mut buf), 0);
        assert_eq!(opt.to_compact(&mut buf), 1);
        assert_eq!(buf.written_len(), 1 + 32);

        assert_eq!(Option::<B256>::from_compact(buf.written_slice(), 1), (opt, vec![].as_slice()));

        // If `None`, it returns the slice at the same cursor position
        assert_eq!(
            Option::<B256>::from_compact(buf.written_slice(), 0),
            (None, buf.written_slice())
        );

        let mut buf = WritableBuffer::with_capacity(32);
        assert_eq!(opt.specialized_to_compact(&mut buf), 1);
        assert_eq!(buf.written_len(), 32);
        assert_eq!(
            Option::<B256>::specialized_from_compact(buf.written_slice(), 1),
            (opt, vec![].as_slice())
        );
    }

    #[test]
    fn test_compact_vec() {
        let list = vec![B256::ZERO, B256::ZERO];
        let mut buf = WritableBuffer::new();

        // Vec doesn't return a total length
        assert_eq!(list.to_compact(&mut buf), 0);

        // Add some noise data that should be returned by `from_compact`
        buf.put_slice(&[1u8, 2]);

        let (decoded_list, remaining_buf) = Vec::<B256>::from_compact(buf.written_slice(), 0);
        assert_eq!(decoded_list, list);
        assert_eq!(remaining_buf, &[1u8, 2]);
    }

    #[test]
    fn compact_u256() {
        let mut buf = vec![];

        assert_eq!(U256::ZERO.to_compact(&mut buf), 0);
        assert!(buf.is_empty());
        assert_eq!(U256::from_compact(&buf, 0), (U256::ZERO, vec![].as_slice()));

        assert_eq!(U256::from(2).to_compact(&mut buf), 1);
        assert_eq!(buf, vec![2u8]);
        assert_eq!(U256::from_compact(&buf, 1), (U256::from(2), vec![].as_slice()));
    }

    #[test]
    fn compact_u64() {
        let mut buf = vec![];

        assert_eq!(0u64.to_compact(&mut buf), 0);
        assert!(buf.is_empty());
        assert_eq!(u64::from_compact(&buf, 0), (0u64, vec![].as_slice()));

        assert_eq!(2u64.to_compact(&mut buf), 1);
        assert_eq!(buf, vec![2u8]);
        assert_eq!(u64::from_compact(&buf, 1), (2u64, vec![].as_slice()));

        let mut buf = Vec::with_capacity(8);

        assert_eq!(0xffffffffffffffffu64.to_compact(&mut buf), 8);
        assert_eq!(&buf, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
        assert_eq!(u64::from_compact(&buf, 8), (0xffffffffffffffffu64, vec![].as_slice()));
    }

    #[test]
    fn variable_uint() {
        proptest::proptest!(|(val: usize)| {
            let mut buf = vec![];
            encode_varuint(val, &mut buf);
            let (decoded, read_buf) = decode_varuint(&buf);
            assert_eq!(val, decoded);
            assert!(!read_buf.has_remaining());
        });
    }

    #[test]
    fn compact_slice() {
        let vec_list = vec![B256::ZERO, B256::random(), B256::random(), B256::ZERO];

        // to_compact
        {
            let mut vec_buf = vec![];
            assert_eq!(vec_list.to_compact(&mut vec_buf), 0);

            let mut slice_buf = vec![];
            assert_eq!(vec_list.as_slice().to_compact(&mut slice_buf), 0);

            assert_eq!(vec_buf, slice_buf);
        }

        // specialized_to_compact
        {
            let mut vec_buf = vec![];
            assert_eq!(vec_list.specialized_to_compact(&mut vec_buf), 0);

            let mut slice_buf = vec![];
            assert_eq!(vec_list.as_slice().specialized_to_compact(&mut slice_buf), 0);

            assert_eq!(vec_buf, slice_buf);
        }
    }

    #[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Compact, arbitrary::Arbitrary)]
    #[add_arbitrary_tests(crate, compact)]
    #[reth_codecs(crate = "crate")]
    struct TestStruct {
        f_u64: u64,
        f_u256: U256,
        f_bool_t: bool,
        f_bool_f: bool,
        f_option_none: Option<B256>,
        f_option_some: Option<B256>,
        f_option_some_u64: Option<u64>,
        f_vec_empty: Vec<Address>,
        f_vec_some: Vec<Address>,
    }

    impl Default for TestStruct {
        fn default() -> Self {
            Self {
                f_u64: 1u64,                                    // 4 bits | 1 byte
                f_u256: U256::from(1u64),                       // 6 bits | 1 byte
                f_bool_f: false,                                // 1 bit  | 0 bytes
                f_bool_t: true,                                 // 1 bit  | 0 bytes
                f_option_none: None,                            // 1 bit  | 0 bytes
                f_option_some: Some(B256::ZERO),                // 1 bit  | 32 bytes
                f_option_some_u64: Some(0xffffu64),             // 1 bit  | 1 + 2 bytes
                f_vec_empty: vec![],                            // 0 bits | 1 bytes
                f_vec_some: vec![Address::ZERO, Address::ZERO], // 0 bits | 1 + 20*2 bytes
            }
        }
    }

    #[test]
    fn test_compact_vec_slice() {
        let vec_list = vec![B256::ZERO, B256::ZERO];

        // to_compact
        {
            let mut vec_buf = WritableBuffer::new();
            assert_eq!(vec_list.to_compact(&mut vec_buf), 0);

            let mut slice_buf = WritableBuffer::new();
            assert_eq!(vec_list.as_slice().to_compact(&mut slice_buf), 0);

            assert_eq!(vec_buf.written_slice(), slice_buf.written_slice());
        }

        // specialized_to_compact
        {
            let mut vec_buf = WritableBuffer::new();
            assert_eq!(vec_list.specialized_to_compact(&mut vec_buf), 0);

            let mut slice_buf = WritableBuffer::new();
            assert_eq!(vec_list.as_slice().specialized_to_compact(&mut slice_buf), 0);

            assert_eq!(vec_buf.written_slice(), slice_buf.written_slice());
        }
    }

    #[test]
    fn test_compact_test_struct() {
        let test = TestStruct::default();
        const EXPECTED_SIZE: usize = 2 + // TestStructFlags
            1 +
            1 +
            // 0 + 0 + 0 +
            32 +
            1 + 2 +
            1 +
            1 + 20 * 2;
        let mut buf = WritableBuffer::with_capacity(EXPECTED_SIZE);
        assert_eq!(test.to_compact(&mut buf), EXPECTED_SIZE);

        assert_eq!(
            TestStruct::from_compact(buf.written_slice(), buf.written_len()),
            (TestStruct::default(), vec![].as_slice())
        );
    }

    #[derive(
        Debug, PartialEq, Clone, Default, Serialize, Deserialize, Compact, arbitrary::Arbitrary,
    )]
    #[add_arbitrary_tests(crate, compact)]
    #[reth_codecs(crate = "crate")]
    enum TestEnum {
        #[default]
        Var0,
        Var1(TestStruct),
        Var2(u64),
    }

    #[cfg(test)]
    #[allow(dead_code)]
    #[test_fuzz::test_fuzz]
    fn compact_test_enum_all_variants(var0: TestEnum, var1: TestEnum, var2: TestEnum) {
        let mut buf = vec![];
        var0.to_compact(&mut buf);
        assert_eq!(TestEnum::from_compact(&buf, buf.len()).0, var0);

        let mut buf = vec![];
        var1.to_compact(&mut buf);
        assert_eq!(TestEnum::from_compact(&buf, buf.len()).0, var1);

        let mut buf = vec![];
        var2.to_compact(&mut buf);
        assert_eq!(TestEnum::from_compact(&buf, buf.len()).0, var2);
    }

    #[test]
    fn test_compact_test_enum() {
        let var0 = TestEnum::Var0;
        let var1 = TestEnum::Var1(TestStruct::default());
        let var2 = TestEnum::Var2(1u64);

        compact_test_enum_all_variants(var0, var1, var2);
    }
}
