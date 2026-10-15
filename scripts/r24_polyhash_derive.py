#!/usr/bin/env python3
"""R24 (Run 500+ Track 1): derive the SPANHASH-64 constants.

Replaces the 9-multiply FNV-1a-64 lane combine (`fnv_lanes_64`) with a
CLMUL-based Carter-Wegman polynomial hash over GF(2^64):

    H(w[0..3], L) = reduce_P( XOR_{j=0..4} clmul128(term_j, K_j) )

where
  * terms are the 4 packed lane-pair words w[j] = lanes[2j] | lanes[2j+1]<<32
    plus L = span length widened to 32 bits (the exact FNV input domain),
  * K_j are FIXED public key powers (determinism law: the golden value must
    be a pure function of the emissions — the CW *construction* gives the
    collision structure, the fixed keys keep every run bit-reproducible),
  * P is a degree-64 IRREDUCIBLE polynomial (searched + proven here),
  * reduce_P is a two-step folding reduction (Barrett-style): since
    x^64 = p(x) mod P, folding x = hi*x^64 + lo -> (hi (x) p) ^ lo drops the
    degree by 64 - deg(p); a sparse p (deg <= 8) reaches < 64 in 2 rounds.

Outputs the Rust constant block for crates/nf-testkit/src/polyhash.rs and
test vectors (cross-checked against polynomial long division).
"""

import random

MASK128 = (1 << 128) - 1


def clmul(a: int, b: int) -> int:
    """Carry-less (GF(2)) polynomial multiplication."""
    r = 0
    while b:
        if b & 1:
            r ^= a
        a <<= 1
        b >>= 1
    return r


def poly_mod(x: int, p: int, deg: int) -> int:
    """x mod p in GF(2)[x] (p of degree `deg`, leading x^deg implied present)."""
    x &= MASK128
    while x.bit_length() > deg:
        x ^= p << (x.bit_length() - 1 - deg)
    return x


def poly_gcd(a: int, b: int) -> int:
    """Euclidean gcd in GF(2)[x]."""
    while b:
        if a.bit_length() < b.bit_length():
            a, b = b, a
        if b == 0:
            break
        a ^= b << (a.bit_length() - b.bit_length())
    return a


def is_irreducible(p: int, deg: int) -> bool:
    """Rabin irreducibility test: p (deg d) is irreducible iff
    x^(2^d) == x (mod p) and gcd(x^(2^(d/q)) - x, p) == 1 for every
    prime q | d."""
    # x^(2^d): d squarings of x
    v = poly_mod(2, p, deg)
    for _ in range(deg):
        v = poly_mod(clmul(v, v), p, deg)
    if v != poly_mod(2, p, deg):
        return False
    # prime factors of d (d = 2^a * odd part)
    d = deg
    primes = set()
    q = 2
    while q * q <= d:
        while d % q == 0:
            primes.add(q)
            d //= q
        q += 1
    if d > 1:
        primes.add(d)
    for q in primes:
        e = deg // q
        w = poly_mod(2, p, deg)
        for _ in range(e):
            w = poly_mod(clmul(w, w), p, deg)
        if poly_gcd(p, w ^ poly_mod(2, p, deg)) not in (1,):
            # gcd must be 1 (the only monic divisor of degree 0)
            g = poly_gcd(p, w ^ poly_mod(2, p, deg))
            if g != 1:
                return False
    return True


# ── Self-test the irreducibility oracle on known ground truth ─────────────
assert is_irreducible((1 << 8) | 0x1B, 8), "AES poly x^8+x^4+x^3+x+1 must be irreducible"
assert not is_irreducible((1 << 8) | (1 << 4), 8), "x^8+x^4 is reducible"
assert is_irreducible((1 << 3) | 0x0B, 3), "x^3+x+1 irreducible (GF(8))"

# Known-good small: x^4+x+1 IS irreducible over GF(2).
assert is_irreducible((1 << 4) | 0x03, 4), "x^4+x+1 must be irreducible"
# Reducible degree-6: (x^3+x+1)^2 = x^6+x^2+1 (squaring doubles exponents)
assert not is_irreducible((1 << 6) | 0x05, 6), "x^6+x^2+1 = (x^3+x+1)^2 reducible"

# ── Modulus search: prefer the 0x1B pentanomial shape, else sparse ───────
CANDIDATES = [(1 << 64) | 0x1B]  # x^64+x^4+x^3+x+1
# sparse pentanomials / trinomials of degree 64
for k1 in (62, 33, 31, 19, 17, 16, 15, 13, 12, 9, 8, 7, 5, 4, 3, 2, 1):
    CANDIDATES.append((1 << 64) | (1 << k1) | 1)  # trinomials x^64+x^k1+1
for k1, k2 in ((4, 3), (3, 1), (5, 3), (6, 1), (7, 1), (8, 3), (9, 7), (12, 3), (19, 3), (31, 30)):
    for k3 in range(1, max(k1, k2)):
        p = (1 << 64) | (1 << k1) | (1 << k2) | (1 << k3) | 1
        CANDIDATES.append(p)

P_POLY = None
for p in CANDIDATES:
    low = p ^ (1 << 64)
    # require deg(p_low) >= 4 so the two-round folding argument is clean,
    # and <= 8 so two rounds always finish from 127 bits
    if not (4 <= low.bit_length() - 1 <= 8):
        continue
    if is_irreducible(p, 64):
        P_POLY = p
        break

assert P_POLY is not None, "no sparse irreducible degree-64 modulus found"
P_LOW = P_POLY ^ (1 << 64)
P_DEG_LOW = P_LOW.bit_length() - 1

# Two-round folding proof: 127-bit residues reduce to < 64 bits in 2 rounds.
def reduce_generic(x: int, rounds: int = 8) -> int:
    for _ in range(rounds):
        if x < (1 << 64):
            break
        hi = x >> 64
        lo = x & ((1 << 64) - 1)
        x = clmul(hi, P_LOW) ^ lo
    return x


random.seed(0x5A5A)
for _ in range(5_000):
    x = random.getrandbits(128) & MASK128
    r = reduce_generic(x)
    assert r < (1 << 64), "two rounds insufficient"
    assert poly_mod(x, P_POLY, 64) == r, "folding != long division"


def gf_mul(a: int, b: int) -> int:
    return poly_mod(clmul(a, b), P_POLY, 64)


def gf_pow(a: int, e: int) -> int:
    r = 1
    while e:
        if e & 1:
            r = gf_mul(r, a)
        a = gf_mul(a, a)
        e >>= 1
    return r


BASE = 0x9E37_79B9_7F4A_7C15  # nothing-up-my-sleeve: the codebase's fold constant
KEYS = [gf_pow(BASE, e) for e in range(1, 6)]  # K0..K4 = r^1..r^5
assert len(set(KEYS)) == 5 and all(k != 0 for k in KEYS), "keys distinct & nonzero"
assert all(bin(k).count("1") >= 16 for k in KEYS), "keys well-mixed"


def reduce_p(x: int) -> int:
    return reduce_generic(x)


def spanhash(words: list, length: int) -> int:
    acc = 0
    terms = list(words) + [length & 0xFFFFFFFF]
    for t, k in zip(terms, KEYS):
        acc ^= clmul(t, k)
    return reduce_p(acc & MASK128)


# ── Emit the Rust constant block + test vectors ───────────────────────────
print(f"// modulus: x^64 + x^{P_DEG_LOW} + ... (low word 0x{P_LOW:X}), proven irreducible (Rabin)")
print("// ── derived by scripts/r24_polyhash_derive.py — do not edit ──")
print(f"pub const SPANHASH_MOD_P_LOW: u64 = 0x{P_LOW:016X};")
print("pub const SPANHASH_KEYS: [u64; 5] = [")
for k in KEYS:
    print(f"    0x{k:016X},")
print("];")

vectors = [
    ([0, 0, 0, 0], 0),
    ([1, 0, 0, 0], 1),
    ([0xFFFFFFFFFFFFFFFF, 0, 0, 0], 0),
    ([0, 0, 0, 0], 0xFFFFFFFF),
    ([0xDEADBEEFCAFEBABE, 0x0123456789ABCDEF, 0xFEDCBA9876543210, 0x13579BDF2468ACE0], 1364),
    ([0x1111111111111111, 0x2222222222222222, 0x3333333333333333, 0x4444444444444444], 8),
    ([0x00000000AABBCCDD, 0xEEFF001122334455, 0x66778899AABBCCDD, 0xEEFF001122334455], 0),
    ([random.getrandbits(64), random.getrandbits(64), random.getrandbits(64), random.getrandbits(64)], random.getrandbits(32)),
    ([random.getrandbits(64), random.getrandbits(64), random.getrandbits(64), random.getrandbits(64)], random.getrandbits(32)),
    ([random.getrandbits(64), random.getrandbits(64), random.getrandbits(64), random.getrandbits(64)], random.getrandbits(32)),
]
print("// test vectors: (w0,w1,w2,w3,len) -> hash")
for w, l in vectors:
    h = spanhash(w, l)
    print(f"(([0x{w[0]:016X}, 0x{w[1]:016X}, 0x{w[2]:016X}, 0x{w[3]:016X}], {l}), 0x{h:016X}),")

print(f"# P low = 0x{P_LOW:016X} (deg {P_DEG_LOW}), keys = {[hex(k) for k in KEYS]}")
