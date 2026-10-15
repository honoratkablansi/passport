// Synthetic test accounts only. Run with contract/node_modules/.bin/tsx.
import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { Contract, ledger, pureCircuits as pure } from '../reference/src/wallet/contract.js';
import { JubjubDevice, K256Device, JubjubGrantee, K256Grantee, SECP256K1_N, authArgs, scopeArgs } from '../reference/src/wallet/signer.js';
const require = createRequire(new URL('../package.json', import.meta.url));
const { createCircuitContext, createConstructorContext } = await import(require.resolve('@midnight-ntwrk/compact-runtime'));
const dir = fileURLToPath(new URL('../fixtures/private/', import.meta.url));
mkdirSync(dir, { recursive: true });
const b = (n: number, width = 32) => new Uint8Array(width).fill(n);
const clean = (x: any): any => x instanceof Uint8Array ? [...x] : typeof x === 'bigint' ? x.toString() : Array.isArray(x) ? x.map(clean) : x && typeof x === 'object' ? Object.fromEntries(Object.entries(x).map(([k, v]) => [k, clean(v)])) : x;
function state(s: any): any {
  if (typeof s.type !== 'function' && s.state) return state(s.state);
  const tag = s.type();
  if (tag === 'null') return { tag };
  if (tag === 'cell') return { tag, value: clean(s.asCell()) };
  if (tag === 'array') return { tag, value: s.asArray().map(state) };
  if (tag === 'map') {
    const m = s.asMap();
    return { tag, value: m.keys().map((k: any) => [clean(k), state(m.get(k))]) };
  }
  throw new Error(`unsupported state ${tag}`);
}
let seq = 0;
const fixtureFiles: string[] = [];
for (const [arm, grantArm, envelope] of [
  ['k256', 'jubjub', 1n], ['k256', 'k256', 1n],
  ['jubjub', 'jubjub', 0n], ['jubjub', 'k256', 0n],
  ['k256', 'jubjub', 0n], ['k256', 'k256', 0n],
] as const) {
  const owner = arm === 'jubjub' ? new JubjubDevice(17n) : new K256Device(17n, envelope);
  const grantee = grantArm === 'jubjub' ? new JubjubGrantee(23n) : new K256Grantee(23n);
  const address = b(11), addressHex = Buffer.from(address).toString('hex');
  const coinPk = { bytes: b(12) }, salt = b(13), color = b(14), recipient = { bytes: b(15) };
  let coin = { nonce: b(16), color, value: 1000n, mt_index: 0n };
  const contract = new Contract({ held_coin: (ctx: any) => [ctx.privateState, coin] });
  const init = await contract.initialState(createConstructorContext({}, coinPk), owner.bootCommitment(salt), b(18));
  let current = init.currentContractState;
  let counter = 0n;
  async function call(name: string, args: any[], label = name) {
    const ctx = createCircuitContext(name, addressHex, coinPk, current, {}, undefined, undefined, undefined, 1_800_000_000);
    const res = await (contract.impureCircuits as any)[name](ctx, ...args);
    const trace = res.context.callProofDataTrace;
    const data = trace[trace.length - 1];
    const fixture = {
      name, label: `${arm}-e${envelope}-${grantArm}-${label}`, address: [...address], time: 1_800_000_000,
      expires_at: name.includes('_with_grant_') && envelope === 1n ? 1_800_000_060 : null,
      before: state(data.initialQueryContext.state), after: state(data.finalQueryContext.state),
      input: clean(data.input), output: clean(data.output),
      witnesses: clean(data.privateTranscriptOutputs),
    };
    const filename = `${String(seq++).padStart(3, '0')}-${label}.json`;
    writeFileSync(`${dir}/${filename}`, JSON.stringify(fixture));
    fixtureFiles.push(filename);
    current = res.context.callContext.currentQueryContext.state;
    return res.result;
  }
  async function device(op: string, args: any[], challengeArgs = args) {
    const challenge = (pure as any)[`challenge_${op}_with_${arm}`];
    const nonce = ledger(current).auth_nonce;
    const a = arm === 'jubjub'
      ? (owner as JubjubDevice).sign((r, grind) => challenge({ bytes: address }, r, owner.pk, ...challengeArgs, nonce, grind), counter)
      : (owner as K256Device).sign(challenge({ bytes: address }, owner.pk, ...challengeArgs, nonce), counter);
    // The reference deliberately accepts both ECDSA S forms.
    if (a.arm === 'k256' && envelope === 1n && counter % 2n === 0n) a.sig.s = SECP256K1_N - a.sig.s;
    const result = await call(`${op}_with_${arm}`, [...args, ...authArgs(a)]);
    counter++;
    return result;
  }
  await call(`activate_initial_device_with_${arm}`, arm === 'jubjub' ? [owner.pk, salt] : [owner.pk, salt, envelope]);
  await call('deposit_unshielded', [color, 10000n], `${arm}-deposit-new`);
  await call('deposit_unshielded', [color, 100n], `${arm}-deposit-existing`);
  await call('deposit_shielded', [{ nonce: coin.nonce, color, value: coin.value }, b(19, 192)], `${arm}-deposit-shielded`);
  await device('append_inbox', [b(20, 192)]);
  await device('rotate_enc_key', [b(21)]);
  const other = arm === 'jubjub' ? new K256Device(29n) : new JubjubDevice(29n);
  const entry = other.entryAt(address, 0n, 0n);
  await device('add_device', [entry]);
  await device('remove_device', [entry]);
  await device('withdraw_unshielded', [color, 10n, recipient]);
  for (const amount of [100n, 1000n]) {
    for (const op of ['withdraw_shielded', 'withdraw_shielded_to_contract']) {
      await device(op, [recipient, color, amount], [recipient, color, amount, coin]);
    }
  }
  const scopeSalt = b(30), origin = b(31), slot = 0n;
  const id = grantee.grantId(address, origin, slot);
  const scope = {
    opWithdrawUnshielded: true, opWithdrawShielded: true, opWithdrawShieldedToContract: true,
    read: true, color, recipientKind: 0n, recipient: b(0), maxCoinValue: 1000n,
    perCallCap: 1000n, cap: 10000n, expiresAt: envelope === 1n ? 1_800_000_060n : 0n, rpIdHash: b(0), readPkHash: b(32),
    windowLen: 0n, windowCap: 0n,
  };
  const scopeList = scopeArgs(scope);
  const digest = (pure as any).derive_grant_scope_digest(scopeSalt, ...scopeList);
  await device('issue_grant', [id, ...scopeList, scopeSalt], [id, digest]);
  let spent = 0n;
  for (const op of ['withdraw_unshielded', 'withdraw_shielded', 'withdraw_shielded_to_contract']) {
    const amounts = op === 'withdraw_unshielded' ? [10n] : [100n, 1000n];
    for (const amount of amounts) {
      const g = ledger(current).grants.lookup(id);
      const entry = b(33, 192), enc = b(21);
      const args = op === 'withdraw_unshielded' ? [color, amount, recipient] : [recipient, color, amount, entry, enc];
      const challengeArgs = [id, g.issued_at, ...args, ...(op === 'withdraw_unshielded' ? [] : [coin]), g.nonce];
      const challenge = (pure as any)[`challenge_${op}_with_grant_${grantArm}`];
      const a: any = grantArm === 'jubjub'
        ? (grantee as JubjubGrantee).sign((r, grind) => challenge({ bytes: address }, r, grantee.pk, ...challengeArgs, grind))
        : (grantee as K256Grantee).sign(challenge({ bytes: address }, grantee.pk, ...challengeArgs));
      if (grantArm === 'k256' && envelope === 1n) a.sig.s = SECP256K1_N - a.sig.s;
      const tail = [grantee.pk, ...(grantArm === 'k256' ? [0n] : []), origin, slot, scopeSalt, 0n, b(0), 1000n, spent,
        ...(grantArm === 'jubjub' ? [a.sig_r, a.sig_s, a.grind_nonce] : [a.sig])];
      await call(`${op}_with_grant_${grantArm}`, [...args, ...tail]);
      spent += amount;
    }
  }
  await device('revoke_grant', [id]);
  await device('revoke_all_grants', []);
}
console.log(`Captured ${seq} synthetic reference executions in ${dir}`);

writeFileSync(`${dir}/index.json`, JSON.stringify(fixtureFiles));
