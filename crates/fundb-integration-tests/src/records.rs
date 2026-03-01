use fundb_core::{FunRecord, FunRecordBuilder, RecordKey};
use uuid::Uuid;

/// Build a minimal FunRecord for the given collection.
pub fn make_record(collection: &str) -> FunRecord {
    FunRecordBuilder::new(collection).build()
}

/// Build a FunRecord with a specific confidence value.
pub fn make_record_with_confidence(collection: &str, confidence: f32) -> FunRecord {
    FunRecordBuilder::new(collection)
        .confidence(confidence)
        .build()
}

/// Build a FunRecord with a named vector embedding.
pub fn make_record_with_vector(collection: &str, name: &str, vec: Vec<f32>) -> FunRecord {
    FunRecordBuilder::new(collection)
        .vector(name, vec)
        .build()
}

/// Build a FunRecord with confidence and a vector.
pub fn make_record_full(
    collection: &str,
    confidence: f32,
    vector_name: &str,
    vec: Vec<f32>,
) -> FunRecord {
    FunRecordBuilder::new(collection)
        .confidence(confidence)
        .vector(vector_name, vec)
        .build()
}

/// Build a FunRecord with custom data payload.
pub fn make_record_with_data(collection: &str, data: Vec<u8>) -> FunRecord {
    FunRecordBuilder::new(collection).data(data).build()
}

/// Build a FunRecord with specific valid-time range.
pub fn make_record_with_valid_time(
    collection: &str,
    valid_from: i64,
    valid_to: i64,
) -> FunRecord {
    FunRecordBuilder::new(collection)
        .valid_time(valid_from, valid_to)
        .build()
}

/// Create a deterministic RecordKey for testing.
pub fn sequential_key(collection: &str, n: u64) -> RecordKey {
    let mut id = [0u8; 16];
    id[8..16].copy_from_slice(&n.to_be_bytes());
    RecordKey {
        collection: collection.to_string(),
        id,
    }
}

/// Create a RecordKey from a FunRecord.
pub fn key_for_record(record: &FunRecord) -> RecordKey {
    RecordKey {
        collection: record._collection.clone(),
        id: *record._id.as_bytes(),
    }
}

/// Generate a random UUID suitable for IDs.
pub fn random_uuid() -> Uuid {
    Uuid::now_v7()
}
