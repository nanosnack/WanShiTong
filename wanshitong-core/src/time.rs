use jiff::Timestamp;
use std::time::UNIX_EPOCH;

pub type UnixTimestampU64 = u64;
pub type UnixTimestampI64 = i64;

pub fn unix_now_u64() -> UnixTimestampU64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn unix_now_i64() -> UnixTimestampI64 {
    unix_now_u64().try_into().unwrap()
}

/// A timestamp as a row stores it, in seconds since the Unix epoch. One out
/// of range, which no row holds short of a bug, reads as the epoch rather
/// than failing the whole query.
pub(crate) fn stored_timestamp(seconds: i64) -> Timestamp {
    Timestamp::from_second(seconds).unwrap_or_default()
}
