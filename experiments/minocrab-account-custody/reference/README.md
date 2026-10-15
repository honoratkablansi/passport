# Pinned comparison oracle

`contracts/account.compact`, `src/wallet/contract.ts`, and `src/wallet/signer.ts`
are byte-for-byte copies from passport commit
`55aa2876edadc5aa873b6693d563d04342e99a88`. They define the 30-entry-point
`spec_version = 2` reference that the initial experiment measured. The original
workspace was at `f519ff0`, whose only additional change was an unrelated MPS
document; the oracle files are identical at both commits.

This snapshot deliberately excludes later caller-bound grants, recovery, and
p256 work. It is test input, not a maintained alternative reference contract.
Do not optimise it alongside the native port: the unchanged oracle is what
allows a meaningful differential comparison.

From the experiment directory, `npm run compile:reference` generates the
ignored `contracts/managed/account` tree and checks all 30 ZKIR hashes against
`provenance.json`. The runtime and signer dependencies are supplied by the
experiment's npm lockfile. No localnet is needed.
