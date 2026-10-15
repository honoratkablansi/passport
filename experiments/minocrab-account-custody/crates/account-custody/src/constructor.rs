//! Initial public state, before the address-bound activation step.
use midnight_base_crypto::fab::{
    AlignedValue, Alignment, AlignmentAtom, AlignmentSegment, Value, ValueAtom,
};
use midnight_onchain_state::state::StateValue;
use midnight_storage::{
    arena::Sp,
    db::InMemoryDB,
    storage::{Array, HashMap},
};

fn cell(bytes: &[u8], width: u32) -> StateValue<InMemoryDB> {
    StateValue::Cell(Sp::new(AlignedValue {
        value: Value(vec![ValueAtom(bytes.to_vec()).normalize()]),
        alignment: Alignment(vec![AlignmentSegment::Atom(AlignmentAtom::Bytes {
            length: width,
        })]),
    }))
}

/// Mirrors the reference constructor. The caller computes the salted boot
/// commitment; a contract address cannot be known at construction time.
pub fn initial_state(boot: [u8; 32], encryption_key: [u8; 32]) -> StateValue<InMemoryDB> {
    StateValue::Array(Array::from(vec![
        cell(&[], 8),
        cell(&encryption_key, 32),
        StateValue::Map(HashMap::new()),
        cell(&[], 8),
        StateValue::Map(HashMap::new()),
        cell(&[2], 4),
        StateValue::Map(HashMap::new()),
        cell(&[], 4),
        cell(&[], 1),
        cell(&[], 8),
        cell(&boot, 32),
        cell(&[], 1),
        StateValue::Map(HashMap::new()),
        cell(&[], 4),
    ]))
}
