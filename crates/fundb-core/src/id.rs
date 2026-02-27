use uuid::Uuid;

/// Generate a new time-ordered UUID v7 record identifier.
///
/// UUID v7 encodes a millisecond-precision Unix timestamp in the most-significant
/// bits, so identifiers sort lexicographically in the same order they were created.
/// This property is critical for the LSM B+Tree (time-ordered ingestion path) and
/// for bitemporal range scans over `_sys_from`.
///
/// # Example
/// ```
/// use fundb_core::new_record_id;
/// let id = new_record_id();
/// assert_eq!(id.get_version(), Some(uuid::Version::SortRand));
/// ```
pub fn new_record_id() -> Uuid {
    Uuid::now_v7()
}
