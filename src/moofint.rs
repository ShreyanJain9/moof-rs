use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Sub, Mul, Div, Rem, Neg};

/// Moof integer: small values stored inline as i64, big values as heap-allocated BigInt.
#[derive(Clone, Debug)]
pub enum MoofInt {
    Small(i64),
    Big(Box<BigInt>),
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

impl MoofInt {
    /// Always returns `Small`.
    pub fn from_i64(n: i64) -> MoofInt {
        MoofInt::Small(n)
    }

    /// Returns `Small` if the value fits in i64, otherwise `Big`.
    pub fn from_bigint(n: BigInt) -> MoofInt {
        match n.to_i64() {
            Some(v) => MoofInt::Small(v),
            None => MoofInt::Big(Box::new(n)),
        }
    }

    /// Shrink a BigInt result back to Small if it fits.
    fn shrink(n: BigInt) -> MoofInt {
        MoofInt::from_bigint(n)
    }

    fn to_bigint(&self) -> BigInt {
        match self {
            MoofInt::Small(v) => BigInt::from(*v),
            MoofInt::Big(b) => (**b).clone(),
        }
    }

    pub fn to_i64(&self) -> Option<i64> {
        match self {
            MoofInt::Small(n) => Some(*n),
            MoofInt::Big(n) => n.to_i64(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

impl MoofInt {
    pub fn is_zero(&self) -> bool {
        match self {
            MoofInt::Small(v) => *v == 0,
            MoofInt::Big(b) => b.is_zero(),
        }
    }

    pub fn is_positive(&self) -> bool {
        match self {
            MoofInt::Small(v) => *v > 0,
            MoofInt::Big(b) => b.is_positive(),
        }
    }

    pub fn is_negative(&self) -> bool {
        match self {
            MoofInt::Small(v) => *v < 0,
            MoofInt::Big(b) => b.is_negative(),
        }
    }

    pub fn is_even(&self) -> bool {
        match self {
            MoofInt::Small(v) => v % 2 == 0,
            MoofInt::Big(b) => b.is_even(),
        }
    }

    pub fn abs(&self) -> MoofInt {
        match self {
            MoofInt::Small(v) => {
                match v.checked_abs() {
                    Some(a) => MoofInt::Small(a),
                    // Only i64::MIN overflows here
                    None => MoofInt::Big(Box::new(BigInt::from(*v).abs())),
                }
            }
            MoofInt::Big(b) => MoofInt::shrink(b.abs()),
        }
    }

    /// Raise to a non-negative exponent.
    ///
    /// Panics if `exp` is negative.
    pub fn pow(&self, exp: &MoofInt) -> MoofInt {
        let e: u64 = match exp {
            MoofInt::Small(v) => {
                assert!(*v >= 0, "MoofInt::pow: exponent must be non-negative");
                *v as u64
            }
            MoofInt::Big(b) => {
                assert!(!b.is_negative(), "MoofInt::pow: exponent must be non-negative");
                b.to_u64().expect("MoofInt::pow: exponent too large")
            }
        };

        // For small base and small exponent, try to stay in i64.
        if let MoofInt::Small(base) = self {
            if e <= 63 {
                let mut result: i64 = 1;
                let mut ok = true;
                for _ in 0..e {
                    match result.checked_mul(*base) {
                        Some(r) => result = r,
                        None => { ok = false; break; }
                    }
                }
                if ok {
                    return MoofInt::Small(result);
                }
            }
        }

        // Fall back to BigInt pow.
        let base_big = self.to_bigint();
        if let Ok(e_usize) = usize::try_from(e) {
            MoofInt::shrink(num_traits::pow::pow(base_big, e_usize))
        } else {
            // Exponent > usize::MAX — repeated squaring manually.
            let mut result = BigInt::one();
            let mut base = base_big;
            let mut remaining = e;
            while remaining > 0 {
                if remaining & 1 == 1 {
                    result *= &base;
                }
                base = &base * &base;
                remaining >>= 1;
            }
            MoofInt::shrink(result)
        }
    }

    pub fn gcd(&self, other: &MoofInt) -> MoofInt {
        let a = self.to_bigint();
        let b = other.to_bigint();
        MoofInt::shrink(a.gcd(&b))
    }

    pub fn to_f64(&self) -> f64 {
        match self {
            MoofInt::Small(v) => *v as f64,
            MoofInt::Big(b) => b.to_f64().unwrap_or(f64::INFINITY),
        }
    }

    /// Integer division (truncating toward zero). Returns `None` on division by zero.
    pub fn checked_div(&self, other: &MoofInt) -> Option<MoofInt> {
        if other.is_zero() {
            return None;
        }
        match (self, other) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_div(*b) {
                    Some(r) => Some(MoofInt::Small(r)),
                    // i64::MIN / -1 overflows
                    None => Some(MoofInt::shrink(BigInt::from(*a) / BigInt::from(*b))),
                }
            }
            _ => {
                Some(MoofInt::shrink(self.to_bigint() / other.to_bigint()))
            }
        }
    }

    /// Integer remainder (truncating toward zero). Returns `None` on division by zero.
    pub fn checked_rem(&self, other: &MoofInt) -> Option<MoofInt> {
        if other.is_zero() {
            return None;
        }
        match (self, other) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_rem(*b) {
                    Some(r) => Some(MoofInt::Small(r)),
                    None => Some(MoofInt::shrink(BigInt::from(*a) % BigInt::from(*b))),
                }
            }
            _ => {
                Some(MoofInt::shrink(self.to_bigint() % other.to_bigint()))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for MoofInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoofInt::Small(v) => write!(f, "{v}"),
            MoofInt::Big(b) => write!(f, "{b}"),
        }
    }
}

// ---------------------------------------------------------------------------
// PartialEq, Eq
// ---------------------------------------------------------------------------

impl PartialEq for MoofInt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MoofInt::Small(a), MoofInt::Small(b)) => a == b,
            (MoofInt::Big(a), MoofInt::Big(b)) => a == b,
            (MoofInt::Small(a), MoofInt::Big(b)) => BigInt::from(*a) == **b,
            (MoofInt::Big(a), MoofInt::Small(b)) => **a == BigInt::from(*b),
        }
    }
}

impl Eq for MoofInt {}

// ---------------------------------------------------------------------------
// PartialOrd, Ord
// ---------------------------------------------------------------------------

impl PartialOrd for MoofInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MoofInt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (MoofInt::Small(a), MoofInt::Small(b)) => a.cmp(b),
            (MoofInt::Big(a), MoofInt::Big(b)) => a.cmp(b),
            (MoofInt::Small(a), MoofInt::Big(b)) => BigInt::from(*a).cmp(b),
            (MoofInt::Big(a), MoofInt::Small(b)) => (**a).cmp(&BigInt::from(*b)),
        }
    }
}

// ---------------------------------------------------------------------------
// Hash — consistent: Small(42) and Big(42) must hash identically
// ---------------------------------------------------------------------------

impl Hash for MoofInt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            MoofInt::Small(v) => {
                state.write_u8(0);
                v.hash(state);
            }
            MoofInt::Big(b) => {
                match b.to_i64() {
                    Some(v) => {
                        // Same path as Small so hashes match
                        state.write_u8(0);
                        v.hash(state);
                    }
                    None => {
                        state.write_u8(1);
                        let (sign, bytes) = b.to_bytes_be();
                        sign.hash(state);
                        bytes.hash(state);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators — owned
// ---------------------------------------------------------------------------

impl Add for MoofInt {
    type Output = MoofInt;

    fn add(self, rhs: MoofInt) -> MoofInt {
        match (&self, &rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_add(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) + BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() + rhs.to_bigint()),
        }
    }
}

impl Sub for MoofInt {
    type Output = MoofInt;

    fn sub(self, rhs: MoofInt) -> MoofInt {
        match (&self, &rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_sub(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) - BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() - rhs.to_bigint()),
        }
    }
}

impl Mul for MoofInt {
    type Output = MoofInt;

    fn mul(self, rhs: MoofInt) -> MoofInt {
        match (&self, &rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_mul(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) * BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() * rhs.to_bigint()),
        }
    }
}

impl Div for MoofInt {
    type Output = MoofInt;

    /// Panics on division by zero. Use `checked_div` for a safe version.
    fn div(self, rhs: MoofInt) -> MoofInt {
        self.checked_div(&rhs).expect("MoofInt: division by zero")
    }
}

impl Rem for MoofInt {
    type Output = MoofInt;

    /// Panics on division by zero. Use `checked_rem` for a safe version.
    fn rem(self, rhs: MoofInt) -> MoofInt {
        self.checked_rem(&rhs).expect("MoofInt: remainder by zero")
    }
}

impl Neg for MoofInt {
    type Output = MoofInt;

    fn neg(self) -> MoofInt {
        match self {
            MoofInt::Small(v) => {
                match v.checked_neg() {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::Big(Box::new(-BigInt::from(v))),
                }
            }
            MoofInt::Big(b) => MoofInt::shrink(-*b),
        }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic operators — by reference (avoids unnecessary cloning)
// ---------------------------------------------------------------------------

impl<'a> Add for &'a MoofInt {
    type Output = MoofInt;

    fn add(self, rhs: &'a MoofInt) -> MoofInt {
        match (self, rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_add(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) + BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() + rhs.to_bigint()),
        }
    }
}

impl<'a> Sub for &'a MoofInt {
    type Output = MoofInt;

    fn sub(self, rhs: &'a MoofInt) -> MoofInt {
        match (self, rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_sub(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) - BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() - rhs.to_bigint()),
        }
    }
}

impl<'a> Mul for &'a MoofInt {
    type Output = MoofInt;

    fn mul(self, rhs: &'a MoofInt) -> MoofInt {
        match (self, rhs) {
            (MoofInt::Small(a), MoofInt::Small(b)) => {
                match a.checked_mul(*b) {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::shrink(BigInt::from(*a) * BigInt::from(*b)),
                }
            }
            _ => MoofInt::shrink(self.to_bigint() * rhs.to_bigint()),
        }
    }
}

impl<'a> Neg for &'a MoofInt {
    type Output = MoofInt;

    fn neg(self) -> MoofInt {
        match self {
            MoofInt::Small(v) => {
                match v.checked_neg() {
                    Some(r) => MoofInt::Small(r),
                    None => MoofInt::Big(Box::new(-BigInt::from(*v))),
                }
            }
            MoofInt::Big(b) => MoofInt::shrink(-(**b).clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<i64> for MoofInt {
    fn from(n: i64) -> Self {
        MoofInt::Small(n)
    }
}

impl From<i32> for MoofInt {
    fn from(n: i32) -> Self {
        MoofInt::Small(n as i64)
    }
}

impl From<BigInt> for MoofInt {
    fn from(n: BigInt) -> Self {
        MoofInt::from_bigint(n)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of(v: &MoofInt) -> u64 {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    }

    #[test]
    fn small_arithmetic() {
        let a = MoofInt::from_i64(10);
        let b = MoofInt::from_i64(3);
        assert_eq!((&a + &b), MoofInt::from_i64(13));
        assert_eq!((&a - &b), MoofInt::from_i64(7));
        assert_eq!((&a * &b), MoofInt::from_i64(30));
        assert_eq!((a.clone() / b.clone()), MoofInt::from_i64(3));
        assert_eq!((a.clone() % b.clone()), MoofInt::from_i64(1));
        assert_eq!((-a), MoofInt::from_i64(-10));
    }

    #[test]
    fn overflow_promotes_to_big() {
        let a = MoofInt::from_i64(i64::MAX);
        let b = MoofInt::from_i64(1);
        let result = a + b;
        assert!(matches!(result, MoofInt::Big(_)));
    }

    #[test]
    fn big_shrinks_back() {
        let big = MoofInt::from_bigint(BigInt::from(42));
        assert!(matches!(big, MoofInt::Small(42)));
    }

    #[test]
    fn equality_across_representations() {
        let a = MoofInt::Small(42);
        let b = MoofInt::Big(Box::new(BigInt::from(42)));
        assert_eq!(a, b);
    }

    #[test]
    fn hash_consistency() {
        let a = MoofInt::Small(42);
        let b = MoofInt::Big(Box::new(BigInt::from(42)));
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    #[test]
    fn ordering() {
        let a = MoofInt::from_i64(-5);
        let b = MoofInt::from_i64(10);
        assert!(a < b);
        assert_eq!(a.cmp(&a), Ordering::Equal);
    }

    #[test]
    fn pow_small() {
        let base = MoofInt::from_i64(2);
        let exp = MoofInt::from_i64(10);
        assert_eq!(base.pow(&exp), MoofInt::from_i64(1024));
    }

    #[test]
    fn pow_promotes_to_big() {
        let base = MoofInt::from_i64(2);
        let exp = MoofInt::from_i64(64);
        let result = base.pow(&exp);
        assert!(matches!(result, MoofInt::Big(_)));
    }

    #[test]
    fn gcd_test() {
        let a = MoofInt::from_i64(12);
        let b = MoofInt::from_i64(8);
        assert_eq!(a.gcd(&b), MoofInt::from_i64(4));
    }

    #[test]
    fn abs_of_min_i64() {
        let a = MoofInt::from_i64(i64::MIN);
        let result = a.abs();
        assert!(result.is_positive());
        assert!(matches!(result, MoofInt::Big(_)));
    }

    #[test]
    fn checked_div_by_zero() {
        let a = MoofInt::from_i64(10);
        let b = MoofInt::from_i64(0);
        assert!(a.checked_div(&b).is_none());
    }

    #[test]
    fn checked_rem_by_zero() {
        let a = MoofInt::from_i64(10);
        let b = MoofInt::from_i64(0);
        assert!(a.checked_rem(&b).is_none());
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", MoofInt::from_i64(-42)), "-42");
        let big = MoofInt::Big(Box::new(BigInt::from(i64::MAX) + BigInt::from(1)));
        assert_eq!(format!("{}", big), "9223372036854775808");
    }

    #[test]
    fn predicates() {
        assert!(MoofInt::from_i64(0).is_zero());
        assert!(!MoofInt::from_i64(1).is_zero());
        assert!(MoofInt::from_i64(5).is_positive());
        assert!(MoofInt::from_i64(-5).is_negative());
        assert!(MoofInt::from_i64(4).is_even());
        assert!(!MoofInt::from_i64(3).is_even());
    }

    #[test]
    fn neg_of_min_i64() {
        let a = MoofInt::from_i64(i64::MIN);
        let result = -a;
        assert!(result.is_positive());
        assert!(matches!(result, MoofInt::Big(_)));
    }

    #[test]
    fn div_min_by_neg_one() {
        // i64::MIN / -1 overflows i64; should promote to Big and shrink
        let a = MoofInt::from_i64(i64::MIN);
        let b = MoofInt::from_i64(-1);
        let result = a.checked_div(&b).unwrap();
        assert!(result.is_positive());
    }
}
