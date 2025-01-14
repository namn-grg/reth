//! Client version model.

use reth_codecs::{add_arbitrary_tests, BufMutWritable, Compact};
use serde::{Deserialize, Serialize};

/// Client version that accessed the database.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "arbitrary"), derive(arbitrary::Arbitrary))]
#[add_arbitrary_tests(compact)]
pub struct ClientVersion {
    /// Client version
    pub version: String,
    /// The git commit sha
    pub git_sha: String,
    /// Build timestamp
    pub build_timestamp: String,
}

impl ClientVersion {
    /// Returns `true` if no version fields are set.
    pub fn is_empty(&self) -> bool {
        self.version.is_empty() && self.git_sha.is_empty() && self.build_timestamp.is_empty()
    }
}

impl Compact for ClientVersion {
    fn to_compact<B>(&self, buf: &mut B) -> usize
    where
        B: BufMutWritable,
    {
        let mut total_length = 0;
        total_length += self.version.to_compact(buf);
        total_length += self.git_sha.to_compact(buf);
        total_length += self.build_timestamp.to_compact(buf);
        total_length
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (version, buf) = String::from_compact(buf, len);
        let (git_sha, buf) = String::from_compact(buf, buf.len());
        let (build_timestamp, buf) = u64::from_compact(buf, buf.len());
        (Self { version, git_sha, build_timestamp }, buf)
    }
}
