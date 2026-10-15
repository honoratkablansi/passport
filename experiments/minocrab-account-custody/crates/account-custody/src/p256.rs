//! P-256 argument ABI and the reference wa-json134 WebAuthn profile.
use crate::auth::{F, Preimage};
use minocrab::v3::{Circuit3, CircuitAbi, Prim, Secp256r1PointT, Secp256r1ScalarT, Wire3};
use minocrab::{AlignmentAtom, Private};
use minocrab_std::v3::*;

#[derive(Clone, Copy)]
pub struct Point(pub Wire3<Secp256r1PointT, Private>);
#[derive(Clone, Copy)]
pub struct Scalar(pub Wire3<Secp256r1ScalarT, Private>);
macro_rules! argument {
    ($name:ident,$ty:ty,$abi:ty) => {
        impl CircuitAbi for $name {
            const SLOTS: usize = 1;
            fn push_atoms(out: &mut Vec<AlignmentAtom>) {
                <$abi as CircuitAbi>::push_atoms(out)
            }
            fn push_prims(out: &mut Vec<Prim>) {
                out.push(Prim::Point)
            }
        }
        impl CircuitArg for $name {
            fn declare(c: &mut Circuit3, path: &ArgPath) -> Self {
                Self(c.arg::<$ty>(path.as_str()))
            }
            fn push_slots(&self, _: &mut Vec<F>) {}
        }
    };
}
argument!(Point, Secp256r1PointT, Secp256k1Point);
argument!(Scalar, Secp256r1ScalarT, Secp256k1Scalar);
#[derive(Clone, Copy, CircuitArg)]
pub struct Signature {
    pub r: Scalar,
    pub s: Scalar,
}
#[derive(Clone, Copy, CircuitArg)]
pub struct Policy {
    pub rp_id_hash: B32<Private>,
    pub origin: Bytes<21>,
}
impl Policy {
    pub fn preimage(&self) -> Preimage {
        Preimage::default()
            .bytes(self.rp_id_hash)
            .uint(self.origin.field(), 21)
    }
}
#[derive(CircuitArg)]
pub struct DeviceAuth {
    pub pk: Point,
    pub use_counter: Uint<64>,
    pub policy: Policy,
    pub authenticator_data: BytesN<Private, 37>,
    pub sig: Signature,
}
#[derive(CircuitArg)]
pub struct GrantAuth {
    pub pk: Point,
    pub policy: Policy,
    pub authenticator_data: BytesN<Private, 37>,
    pub sig: Signature,
}
impl DeviceAuth {
    pub fn into_auth(self, c: &mut Circuit3) -> (crate::auth::Auth, Uint<64>) {
        (
            crate::auth::Auth::p256(c, self.pk, self.policy, self.authenticator_data, self.sig),
            self.use_counter,
        )
    }
}
impl GrantAuth {
    pub fn into_auth(self, c: &mut Circuit3) -> crate::auth::Auth {
        crate::auth::Auth::p256(c, self.pk, self.policy, self.authenticator_data, self.sig)
    }
}

fn base64(c: &mut Circuit3, challenge: B32<Private>) -> BytesN<Private, 43> {
    let bytes = BytesN::from(challenge).to_le_bytes(c);
    let mut bits = Vec::with_capacity(258);
    for byte in bytes {
        let mut value = byte;
        let mut little = Vec::with_capacity(8);
        for _ in 0..8 {
            let (q, r) = c.div_mod_power_of_two(value, 1);
            little.push(r);
            value = q;
        }
        bits.extend(little.into_iter().rev());
    }
    bits.extend([c.constant(0u64).private(); 2]);
    let out: Vec<_> = bits
        .as_chunks::<6>()
        .0
        .iter()
        .map(|bits| {
            let mut index = c.constant(0u64).private();
            for bit in bits {
                let doubled = c.mul(index, 2u64);
                index = c.add(doubled, *bit);
            }
            let bound = c.constant(26u64).private();
            let less26 = c.less_than(index, bound, 6);
            let bound = c.constant(52u64).private();
            let less52 = c.less_than(index, bound, 6);
            let bound = c.constant(62u64).private();
            let less62 = c.less_than(index, bound, 6);
            let eq62 = c.test_eq(index, 62u64);
            let upper = c.add(index, 65u64);
            let lower = c.add(index, 71u64);
            let four = c.constant(4u64).private();
            let minus4 = c.neg(four);
            let digit = c.add(index, minus4);
            let dash = c.constant(45u64).private();
            let underscore = c.constant(95u64).private();
            let tail = c.cond_select(eq62, dash, underscore);
            let tail = c.cond_select(less62, digit, tail);
            let tail = c.cond_select(less52, lower, tail);
            c.cond_select(less26, upper, tail)
        })
        .collect();
    BytesN::from_le_bytes(c, &out)
}

pub fn verify(
    c: &mut Circuit3,
    challenge: B32<Private>,
    policy: &Policy,
    data: &BytesN<Private, 37>,
    sig: Signature,
    pk: Point,
) {
    let bytes = data.to_le_bytes(c);
    let rp = BytesN::<Private, 32>::from_le_bytes(c, &bytes[..32]).to_b32();
    let matches = crate::auth::bytes_eq(c, rp, policy.rp_id_hash);
    c.assert(matches);
    let flags = bytes[32];
    let f5 = c.test_eq(flags, 5u64);
    let f13 = c.test_eq(flags, 13u64);
    let f29 = c.test_eq(flags, 29u64);
    let flags_ok = c.add(f5, f13);
    let flags_ok = c.add(flags_ok, f29);
    c.assert(flags_ok);
    let mut origin = policy.origin.field();
    for _ in 0..21 {
        let (q, b) = c.div_mod_power_of_two(origin, 8);
        origin = q;
        let lower = c.constant(33u64).private();
        let upper = c.constant(126u64).private();
        let low = c.less_than(b, lower, 8);
        let high = c.less_than(upper, b, 8);
        let quote = c.test_eq(b, 34u64);
        let slash = c.test_eq(b, 92u64);
        for invalid in [low, high, quote, slash] {
            let valid = c.not(invalid);
            c.assert(valid);
        }
    }
    let encoded = base64(c, challenge);
    let prefix = BytesN::<Private, 36>::literal(c, b"{\"type\":\"webauthn.get\",\"challenge\":\"");
    let middle = Bytes::<12, minocrab::Public>::constant(c, b"\",\"origin\":\"");
    let suffix = Bytes::<22, minocrab::Public>::constant(c, b"\",\"crossOrigin\":false}");
    let client = Preimage::default()
        .blob(&prefix)
        .blob(&encoded)
        .uint(middle.field().private(), 12)
        .uint(policy.origin.field(), 21)
        .uint(suffix.field().private(), 22)
        .hash(c);
    let digest = Preimage::default().blob(data).bytes(client).hash(c);
    let typed = digest.to_typed(c);
    let reversed = c.reverse_bytes(typed);
    let z: Wire3<Secp256r1ScalarT, Private> = c.from_bytes32(reversed);
    let empty = B32::pad(c, "").private().to_typed(c);
    let zero: Wire3<Secp256r1ScalarT, Private> = c.from_bytes32(empty);
    for scalar in [sig.r.0, sig.s.0] {
        let bad = c.test_eq(scalar, zero);
        let good = c.not(bad);
        c.assert(good);
    }
    let w = c.inv(sig.s.0);
    let u1 = c.mul(z, w);
    let u2 = c.mul(sig.r.0, w);
    let generator = generator(c);
    let a = c.ec_mul(generator, u1);
    let b = c.ec_mul(pk.0, u2);
    let point = c.add(a, b);
    let (x, _) = c.into_coordinates(point);
    let x = c.into_bytes32(x);
    let reduced: Wire3<Secp256r1ScalarT, Private> = c.from_bytes32(x);
    c.assert_eq(reduced, sig.r.0);
}

// SEC 2 P-256 generator, constructed from fixed affine coordinates because
// this Minocrab revision's ec_mul_generator convenience trait covers k1/JubJub.
fn generator(c: &mut Circuit3) -> Wire3<Secp256r1PointT, Private> {
    fn coordinate(c: &mut Circuit3, hex: &str) -> Wire3<minocrab::v3::Secp256r1BaseT, Private> {
        let bytes: Vec<_> = (0..32)
            .rev()
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
            .collect();
        let value = BytesN::<Private, 32>::literal(c, &bytes)
            .to_b32()
            .to_typed(c);
        c.from_bytes32(value)
    }
    let x = coordinate(
        c,
        "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
    );
    let y = coordinate(
        c,
        "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
    );
    c.from_coordinates(x, y)
}
