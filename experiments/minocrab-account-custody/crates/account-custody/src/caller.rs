//! Midnight kernel.caller ABI, missing from the pinned Minocrab convenience API.
//! Read context[6], wrapping its nullable PublicAddress in a canonical Maybe.
//! These are the ledger VM opcodes, not a private caller witness.
use minocrab::v3::{Circuit3, FieldT, ImpactElem, Wire3};
use minocrab::{Fr, Public};

pub fn caller(c: &mut Circuit3) -> [Wire3<FieldT, Public>; 6] {
    let answer = std::array::from_fn(|_| c.public_transcript_input::<FieldT>());
    // dup context; idx caller; dup; type; push cell-type; eq; branch.
    // Present: prefix true to PublicAddress. Absent: canonical zero payload.
    for op in [
        &[0x32][..],
        &[0x60, 1, 1, 6],
        &[0x30],
        &[3],
        &[0x10, 1, 1, 1, 1],
        &[2],
        &[0x12, 4],
        &[0x10, 1, 1, 1, 1],
        &[0x40],
        &[0x16, 0x50],
        &[0x13, 2],
        &[0x0b],
        &[0x10, 1, 4, 1, 1, 32, 32, 0, 0, 0, 0, 0, 0],
    ] {
        let elems: Vec<_> = op
            .iter()
            .map(|&n| ImpactElem::Imm(Fr::from(n as u64)))
            .collect();
        c.impact_mixed::<Public>(1u64, &elems);
    }
    let mut equality: Vec<_> = [0x0d, 4, 1, 1, 32, 32]
        .into_iter()
        .map(|n| ImpactElem::Imm(Fr::from(n as u64)))
        .collect();
    equality.extend(answer.iter().copied().map(ImpactElem::Wire));
    c.impact_mixed::<Public>(1u64, &equality);
    c.acknowledge_stale(&answer.iter().map(|w| w.val()).collect::<Vec<_>>());
    answer
}
