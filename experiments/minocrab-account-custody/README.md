# Account custody in Minocrab

An independent Rust/Minocrab implementation of the **30-entry-point,
`spec_version = 2` account custody reference** at passport commit
[`55aa2876`](https://github.com/midnightntwrk/passport/tree/55aa2876edadc5aa873b6693d563d04342e99a88/contract).
It covers the constructor, JubJub and k256 device authorisation, cross-arm
enrolment, shielded custody, unshielded balance mirrors, encrypted inbox,
and scoped grants. The implementation does not translate Compact instructions;
the pinned Compact implementation is the independent comparison oracle.

**Scope:** this is the historical reference used when the experiment started.
The snapshot in `reference/` makes that comparison reproducible on current
main, whose reference has since gained caller-bound grants and recovery.
Those additions and the separate p256 implementation are outside this port.
No files in the live `contract/` tree are changed by this experiment.

This is an offline circuit experiment, not a deployment package. Constructor
state is reproduced independently, but verification-key installation, wallet
integration, funded transactions, and on-node conformance remain future work.

## What the optimisations demonstrate

The initial Rust port retained the reference's `k` on all 30 entry points.
Its [baseline measurements](evidence/baseline/report.md) are retained separately.
The optimised implementation applies three local changes:

1. **Reuse k256 coordinate bytes.** Derive the canonical byte encodings once
   from the point argument and reuse them in weak-key rejection, commitments,
   and challenges. The bytes are never supplied as independent witnesses.
2. **Specialise grant signature verification.** Spend grants already require
   envelope zero. Retain that assertion and omit the unreachable connector
   digest. Device authorisation continues to support both envelopes.
3. **Reuse shielded output commitments.** Hash the sent coin once and reuse
   its commitment on self-transfers. Specialise the recipient arm at circuit
   construction, retaining nullifier claims, self-receive claims, change
   outputs, checked subtraction, and the zero payload for absent change.

The k256 unshielded grant withdrawal drops from **`k=17` to `k=16`**.
On the recorded Apple M4 Max run, its median proving time falls from **4.40 s
to 2.43 s (45%)**. Proof size stays at **6,336 bytes**, and median cryptographic
verification is **3.48 / 3.71 ms** (Compact / Minocrab); no verification-speed
gain is claimed. A generated `max_k = 16` test prevents regression. The other
savings and real proving/verification samples are in the [paired report](evidence/report.md),
[raw measurements](evidence/benchmark.json), and [cost table](evidence/costs.json).

These changes are not all exclusive to Minocrab. Coordinate reuse and grant
envelope specialisation can also be expressed in Compact. Improving and
validating the live Compact reference is a separate work stream. This comparison measures an
optimised native Rust implementation against the pinned, unchanged reference.

`k` is not determined by the displayed ordinary-row count alone: chip/table
configuration and reserved rows matter, and ordinary-row cost can itself change
when configured at another `k`. A smaller `k` primarily targets proving time and
memory; it does not guarantee a smaller proof or faster verification.

## Parity evidence

[Execution evidence](evidence/equivalence.json) records **150 executions** of
the reference's generated TypeScript, replayed separately through both circuits
and Midnight's Impact VM. Post-state, outputs, and kernel effects agree, and
both proof preimages pass Midnight's own `Zkir::check`. Input/output ABI and
communications-commitment mode match across the complete circuit registry.
The native constructor state is checked against reference initial states.

Coverage includes both owner arms × both grantee arms, cross-arm enrolment,
connector envelopes, both ECDSA S forms, shielded exact spends and change,
external contract recipients, and self-transfers. The harness also passes
**3,120 single-slot input/witness mutations** and **304 context checks** for
replay, wrong-account authorisation, and grant expiry boundaries. There are
33 Rust tests, including disclosure checks, the `k` budget, and executor
regressions. This is differential evidence, not a formal equivalence proof or
the full on-node conformance suite.

The circuits can reorder ledger reads. Each proves its own transcript of the
same operation against the same initial state and arguments; public-input
vectors and proving keys are not claimed to be interchangeable. The historical
reference's documented revocation limitations are preserved, not silently fixed.

## Measurement limits

**Complete transaction sizes are not measured.** `transaction_bytes` is `null`.
Proof bytes and ZKIR JSON bytes are separately labelled and must not be read as
transaction size. A funded transaction also carries transcripts, shielded
proofs/offers, unshielded offers, fee funding, and signatures. Verification
samples measure cryptographic proof verification, excluding ledger replay,
transaction validation, and network latency.

Each paired measurement uses a release build in a fresh subprocess, alternating
which implementation runs first. Parameter loading precedes the key-generation
timer. Key generation is timed separately; one proof warms the process and
three proofs are timed, retaining every proving and verification sample.
The report uses medians. Avoid concurrent CPU-heavy work; three samples do not
support conclusions about small timing differences.

The last successful fixture per circuit supplies the benchmark preimage:
shielded withdrawals use the exact-spend branch and an external recipient.
Self-transfers and change are covered by parity checks but are not separately
timed. Baseline and optimised timings come from separate runs; the new paired
reference/Minocrab measurements are the primary timing comparison.

## Reproduce

Prerequisites: Rust, Node.js/npm, Python 3, and Compact compiler `0.33.0-rc.2`.
From this directory:

```sh
npm ci
npm run compile:reference
npm run fixtures
cargo fmt --all -- --check
cargo clippy -p project-account-custody -p project-custody-measure --all-targets -- -D warnings
cargo test -p project-account-custody -p project-custody-measure
cargo build --release -p project-custody-measure
target/release/project-custody-measure compare --mutations
python3 scripts/bench.py
# Resume only with the same binary, lockfile, equivalence evidence and inputs.
python3 scripts/bench.py --resume
```

`compile:reference` verifies source and generated ZKIR hashes against
[reference provenance](reference/provenance.json). The benchmark refuses stale
binaries or preimages and records their hashes. Dependencies are pinned in the
Rust/npm lockfiles; [run provenance](evidence/provenance.json) identifies the
validated sources and toolchain. Initial builds need network access. Midnight
parameters use `MIDNIGHT_PP` or the normal parameter cache and are downloaded
before timing if missing. Fixtures contain synthetic private witnesses and are
ignored by Git, as are generated circuits, preimages, keys, and build outputs.

## Layout and executor compatibility

- `crates/account-custody/src/`: native ledger schema, constructor, authorisation,
  custody, and grant circuits.
- `reference/`: unchanged Compact source and reference signer/ABI wrapper from
  the pinned passport commit; generated code lives in its ignored `managed/`.
- `scripts/fixtures.ts`: captures reference executions using synthetic accounts.
- `bin/measure/`: ABI gate, differential execution, costs, and proof measurements.
- `evidence/`: current results; `evidence/baseline/` retains the initial port's
  results, original hashes, and original toolchain metadata. Its source is
  available in the first commit of this PR (`6f9739b`). Historical paths in its
  provenance describe the original workspace, not dependencies on current main.

`bin/measure/src/executor.rs` is vendored from Minocrab commit
`537721ae7cd43ca7b3db09db784eb0f35ff437f4`, under its MIT licence in
`vendor/LICENSE-MIT`. It normalises FAB atoms after `parse_field_repr`: that
parser returns padded byte atoms, while the ledger's conversion from a
`ValueAtom` reference to `()` requires an empty atom for an unused union arm.
Without this fix the Compact reference itself fails unshielded deposit with
`InvalidBuiltinDecode("()")`. Two regression tests cover the fix. Other executor
changes are imports, formatting, and tests; circuit constraints and proving
code are unaffected by normalisation.

Next acceptance gate for end-to-end comparison: funded native transaction
construction and localnet conformance, including receiving-contract composition.
Only then populate complete transaction sizes and full validation timings.
