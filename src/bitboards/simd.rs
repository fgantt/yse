use serde::{Deserialize, Serialize, Serializer, Deserializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SimdBitboard {
    data: u128,
}

impl SimdBitboard {
    #[inline(always)]
    pub const fn empty() -> Self {
        Self { data: 0 }
    }

    #[inline(always)]
    pub const fn new(data: u128) -> Self {
        Self::from_u128(data)
    }

    #[inline(always)]
    pub const fn from_u128(value: u128) -> Self {
        Self { data: value }
    }

    #[inline(always)]
    pub fn to_u128(&self) -> u128 {
        self.data
    }

    #[inline(always)]
    pub fn all_squares() -> Self {
        Self { data: 0x1FFFFFFFFFFFFFFFFFFFFFFFF }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.data == 0
    }

    #[inline(always)]
    pub fn count_ones(&self) -> u32 {
        self.data.count_ones()
    }

    #[inline(always)]
    pub fn trailing_zeros(&self) -> u32 {
        self.data.trailing_zeros()
    }

    #[inline(always)]
    pub fn leading_zeros(&self) -> u32 {
        self.data.leading_zeros()
    }
}

impl Default for SimdBitboard {
    #[inline(always)]
    fn default() -> Self {
        Self::empty()
    }
}

impl std::ops::BitAnd for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self { data: self.data & rhs.data }
    }
}

impl std::ops::BitOr for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self { data: self.data | rhs.data }
    }
}

impl std::ops::BitXor for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self { data: self.data ^ rhs.data }
    }
}

impl std::ops::Not for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn not(self) -> Self::Output {
        Self { data: !self.data }
    }
}

impl std::ops::Shl<u32> for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn shl(self, rhs: u32) -> Self::Output {
        Self::from_u128(self.to_u128() << rhs)
    }
}

impl std::ops::Shr<u32> for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn shr(self, rhs: u32) -> Self::Output {
        Self::from_u128(self.to_u128() >> rhs)
    }
}

impl std::ops::BitAndAssign for SimdBitboard {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl std::ops::BitOrAssign for SimdBitboard {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl std::ops::BitXorAssign for SimdBitboard {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl Serialize for SimdBitboard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_u128().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SimdBitboard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u128::deserialize(deserializer)?;
        Ok(SimdBitboard::from_u128(val))
    }
}

impl std::hash::Hash for SimdBitboard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_u128().hash(state);
    }
}

impl PartialOrd for SimdBitboard {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SimdBitboard {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_u128().cmp(&other.to_u128())
    }
}
