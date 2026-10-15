# Account custody in Minocrab

Independent Rust/Minocrab port of `../../contract/contracts/account.compact`:
the constructor state, all 30 state-changing entry points, JubJub and k256
device authorisation, cross-arm enrolment, stateless shielded custody,
unshielded balance mirrors, encrypted inbox, and scoped grants. The Rust
implementation does not import or translate Compact's circuit instructions.
The Compact artifacts are the comparison oracle only.

This is an **offline circuit experiment**, not yet a replacement deployment
package. The independent Rust constructor builds the initial ledger state;
installing verification keys and integrating transaction construction with a
wallet remain deployment work. Pure signing/challenge helpers are reproduced
inside the Rust circuits where used, while fixture generation deliberately
uses the existing reference TypeScript signers as an independent oracle.

## Results and limits

On the Apple M4 Max used for this run, all 30 entry points retain the same
`k` and proof byte count. Timings are close; three samples per implementation
are insufficient to claim a meaningful speedup. For example:

| Unshielded withdrawal | Compact / Minocrab k | Prove seconds | Verify milliseconds | Proof bytes |
|---|---|---|---|---|
| JubJub device | 15 / 15 | 1.130 / 1.132 | 3.14 / 3.25 | 5744 / 5744 |
| k256 device | 16 / 16 | 2.394 / 2.354 | 3.45 / 3.45 | 6336 / 6336 |

- [Cost table](evidence/costs.json): rows and `k` for both implementations,
  using the same pinned Midnight cost model. `k` is unchanged on all 30
  entry points in the initial port; fewer rows do not necessarily lower `k`.
- [Execution evidence](evidence/equivalence.json): reference executions
  captured from the existing generated TypeScript, then replayed separately
  through both circuits and Midnight's Impact VM. Final state, outputs, and
  kernel effects must agree. Both proof preimages must pass Midnight's own
  `Zkir::check`. Covers both owner arms × both grantee arms, cross-arm device
  enrolment, shielded exact spends/change, connector envelopes, and high-S
  ECDSA signatures. All **126 executions** pass.
- `compare --mutations` changes every argument/witness field slot individually
  and requires matching acceptance and, when accepted, matching outcomes.
  All **2,416 mutations** pass, alongside **248 context checks** covering
  replay, wrong-account authorisation, and grant expiry boundaries.
  This is differential testing, not a
  formal equivalence proof or the full on-node conformance suite.
- [Benchmark results](evidence/benchmark.json) and [table](evidence/report.md)
  are produced by the benchmark script. Proving and verification use real
  cryptographic proofs, not estimates or HTTP request duration.
- **Complete transaction sizes are not measured yet.** `transaction_bytes`
  stays `null`. Proof bytes and ZKIR JSON bytes are separate metrics and must
  not be presented as transaction bytes. A funded transaction also includes
  transcripts, shielded offers/proofs, unshielded offers, fee funding, and
  signatures. Likewise, proof verification time excludes transaction
  validation, ledger execution, and network latency.

Consequently this establishes a reproducible circuit-level comparison, but
does not yet complete the requested end-to-end transaction comparison.

## Reproduce

Prerequisites: Rust, Python 3, the reference `contract/node_modules` installation,
and the pinned Compact compiler. Run commands from this directory:

```sh
# Check or recreate the pinned reference artifacts separately; no reference edits.
compact compile +0.33.0-rc.2 --feature-zkir-v3 --skip-zk \
  ../../contract/contracts/account.compact artifacts/reference-fresh

# Synthetic accounts only; generated fixture files include private witness values.
npm run fixtures
cargo test -p project-account-custody
cargo build --release -p project-custody-measure
target/release/project-custody-measure compare --mutations

# Sequential paired measurements: one warm-up and three timed proofs per side.
python3 scripts/bench.py
# Resume only with the same binary, lockfile, inputs, and configuration.
python3 scripts/bench.py --resume

cargo fmt --all -- --check
cargo clippy -p project-account-custody -p project-custody-measure -- -D warnings
cargo test -p project-custody-measure
```

The first dependency build needs network access. The prover uses Midnight's
parameter cache (`MIDNIGHT_PP`, or the normal cache location); missing parameter
files are downloaded before the key-generation timer starts.

Each benchmark runs in a fresh subprocess, alternating which implementation
goes first. Key generation is timed separately; one proof warms the process;
all measured proving and verification samples are retained. Avoid concurrent
CPU-heavy work. The report uses medians and records the machine, binary hash,
lockfile hash, and iteration count. The final successful fixture for each circuit
supplies its benchmark preimage; shielded withdrawal benchmarks therefore use
the exact-spend branch. Branch-specific timings should be added before drawing
conclusions about change-bearing transactions.

The two circuits may reorder ledger reads. They prove their respective
transcripts of the same operation against the same initial state and arguments;
this does **not** claim equal public-input vectors or interchangeable proving
keys. Their communicated arguments and return values remain ABI-compatible.

The recorded benchmark preceded the constructor test and expanded fixture
matrix. Its actual binary and lockfile hashes remain in `benchmark.json`;
[benchmark input hashes](evidence/benchmark-inputs.json) identify the native
circuits and proof preimages used. Final validation confirms all 30 native
circuit files are byte-for-byte identical to those measured. The final lockfile
hash in `provenance.json` reflects the constructor's additional direct
dependencies; the dependency revisions are unchanged. New benchmark runs also
record input hashes directly in their configuration and reject changes during
measurement. [Toolchain versions](evidence/toolchain.json) accompany the results.

## Layout

| Path | Purpose |
|---|---|
| `crates/account-custody/src/lib.rs` | Ledger schema, deposits, circuit registry |
| `crates/account-custody/src/constructor.rs` | Independent initial state |
| `crates/account-custody/src/auth.rs` | FAB hashing, device commitments, both signature schemes |
| `crates/account-custody/src/custody.rs` | Shared custody operations and witness binding |
| `crates/account-custody/src/entrypoints.rs` | Device circuit entry points |
| `crates/account-custody/src/grants.rs` | Grant identity, scope, lifecycle, and settlement |
| `crates/account-custody/src/grant_entrypoints.rs` | Grant circuit entry points |
| `scripts/fixtures.ts` | Capture real reference execution and synthetic inputs |
| `bin/measure/` | ABI coverage gate, differential execution, cost/proof measurements |
| `evidence/provenance.json` | Reference/compiler/dependency pins and artifact hashes |

## Executor compatibility fix

`bin/measure/src/executor.rs` is vendored from Minocrab commit
`537721ae7cd43ca7b3db09db784eb0f35ff437f4`, under its MIT licence
(`vendor/LICENSE-MIT`). It uses the upstream simulator and Midnight VM.
The only behavioural change normalises FAB atoms after `parse_field_repr`:
that parser returns padded byte atoms, while the ledger's `TryFrom<ValueAtom>
for ()` requires an empty atom for the unused half of a token/recipient union.
Without normalisation **the Compact reference itself** fails unshielded deposit
in the executor with `InvalidBuiltinDecode("()")`. The fix changes neither
circuit, hashes, constraints, nor proving code. Other changes are imports and
formatting to embed the module.

## What parity must preserve

The port retains the reference's behaviour, including documented limitations;
this experiment does not silently fix its device revocation design. In
particular: entry-based enrolment/removal, checked integer overflow, exact FAB
hash encodings and domain separators, both ECDSA S forms, envelope selection,
weak-key rejection, ordered held-coin witness binding, independent grant/device
nonces, spent commitments, grant epochs/generations, expiry, recipient pins,
reserved window bounds, and change-entry insertion all belong to the comparison.

Next acceptance gate: native transaction construction and funded localnet
execution against the reference conformance suites, including direct transfers
to a receiving custody contract. Only then populate complete transaction byte
sizes and report full transaction verification times.
