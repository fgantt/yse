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
        // Uses hardware popcount when available (POPCNT on x86_64, similar on ARM64)
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

// SIMD implementations for bitwise operations
// When the `simd` feature is enabled, use explicit SIMD intrinsics
// Otherwise, fall back to scalar u128 operations

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
mod x86_64_simd {
    use super::SimdBitboard;
    use std::arch::x86_64::*;

    #[inline(always)]
    pub(super) fn bitand(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            // Load u128 bytes directly into SSE register
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = _mm_loadu_si128(a_bytes.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b_bytes.as_ptr() as *const __m128i);
            
            // Perform SIMD AND
            let result = _mm_and_si128(a_vec, b_vec);
            
            // Extract result back to u128
            let mut result_bytes = [0u8; 16];
            _mm_storeu_si128(result_bytes.as_mut_ptr() as *mut __m128i, result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn bitor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = _mm_loadu_si128(a_bytes.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b_bytes.as_ptr() as *const __m128i);
            
            let result = _mm_or_si128(a_vec, b_vec);
            
            let mut result_bytes = [0u8; 16];
            _mm_storeu_si128(result_bytes.as_mut_ptr() as *mut __m128i, result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn bitxor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = _mm_loadu_si128(a_bytes.as_ptr() as *const __m128i);
            let b_vec = _mm_loadu_si128(b_bytes.as_ptr() as *const __m128i);
            
            let result = _mm_xor_si128(a_vec, b_vec);
            
            let mut result_bytes = [0u8; 16];
            _mm_storeu_si128(result_bytes.as_mut_ptr() as *mut __m128i, result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn not(a: SimdBitboard) -> SimdBitboard {
        unsafe {
            // Create all-ones mask
            let ones = _mm_set1_epi8(-1i8);
            let a_bytes = a.to_u128().to_le_bytes();
            let a_vec = _mm_loadu_si128(a_bytes.as_ptr() as *const __m128i);
            
            let result = _mm_andnot_si128(a_vec, ones);
            
            let mut result_bytes = [0u8; 16];
            _mm_storeu_si128(result_bytes.as_mut_ptr() as *mut __m128i, result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn shl(a: SimdBitboard, shift: u32) -> SimdBitboard {
        // For shifts, we can use SIMD but need to handle cross-lane shifts carefully
        // For simplicity and correctness, use scalar for now (can be optimized later)
        SimdBitboard::from_u128(a.to_u128() << shift)
    }

    #[inline(always)]
    pub(super) fn shr(a: SimdBitboard, shift: u32) -> SimdBitboard {
        SimdBitboard::from_u128(a.to_u128() >> shift)
    }
}

#[cfg(all(feature = "simd", target_arch = "aarch64"))]
mod aarch64_simd {
    use super::SimdBitboard;
    use std::arch::aarch64::*;

    #[inline(always)]
    pub(super) fn bitand(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = vld1q_u8(a_bytes.as_ptr());
            let b_vec = vld1q_u8(b_bytes.as_ptr());
            
            let result = vandq_u8(a_vec, b_vec);
            
            let mut result_bytes = [0u8; 16];
            vst1q_u8(result_bytes.as_mut_ptr(), result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn bitor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = vld1q_u8(a_bytes.as_ptr());
            let b_vec = vld1q_u8(b_bytes.as_ptr());
            
            let result = vorrq_u8(a_vec, b_vec);
            
            let mut result_bytes = [0u8; 16];
            vst1q_u8(result_bytes.as_mut_ptr(), result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn bitxor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let b_bytes = b.to_u128().to_le_bytes();
            
            let a_vec = vld1q_u8(a_bytes.as_ptr());
            let b_vec = vld1q_u8(b_bytes.as_ptr());
            
            let result = veorq_u8(a_vec, b_vec);
            
            let mut result_bytes = [0u8; 16];
            vst1q_u8(result_bytes.as_mut_ptr(), result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn not(a: SimdBitboard) -> SimdBitboard {
        unsafe {
            let a_bytes = a.to_u128().to_le_bytes();
            let a_vec = vld1q_u8(a_bytes.as_ptr());
            
            // Create all-ones mask and XOR
            let ones = vdupq_n_u8(0xFF);
            let result = veorq_u8(a_vec, ones);
            
            let mut result_bytes = [0u8; 16];
            vst1q_u8(result_bytes.as_mut_ptr(), result);
            SimdBitboard::from_u128(u128::from_le_bytes(result_bytes))
        }
    }

    #[inline(always)]
    pub(super) fn shl(a: SimdBitboard, shift: u32) -> SimdBitboard {
        SimdBitboard::from_u128(a.to_u128() << shift)
    }

    #[inline(always)]
    pub(super) fn shr(a: SimdBitboard, shift: u32) -> SimdBitboard {
        SimdBitboard::from_u128(a.to_u128() >> shift)
    }
}

// Scalar fallback implementations (when simd feature is disabled or on unsupported platforms)
#[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
mod scalar_fallback {
    use super::SimdBitboard;

    #[inline(always)]
    pub(super) fn bitand(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        SimdBitboard { data: a.data & b.data }
    }

    #[inline(always)]
    pub(super) fn bitor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        SimdBitboard { data: a.data | b.data }
    }

    #[inline(always)]
    pub(super) fn bitxor(a: SimdBitboard, b: SimdBitboard) -> SimdBitboard {
        SimdBitboard { data: a.data ^ b.data }
    }

    #[inline(always)]
    pub(super) fn not(a: SimdBitboard) -> SimdBitboard {
        SimdBitboard { data: !a.data }
    }

    #[inline(always)]
    pub(super) fn shl(a: SimdBitboard, shift: u32) -> SimdBitboard {
        SimdBitboard::from_u128(a.to_u128() << shift)
    }

    #[inline(always)]
    pub(super) fn shr(a: SimdBitboard, shift: u32) -> SimdBitboard {
        SimdBitboard::from_u128(a.to_u128() >> shift)
    }
}

// Public trait implementations that dispatch to SIMD or scalar implementations
impl std::ops::BitAnd for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::bitand(self, rhs)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::bitand(self, rhs)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::bitand(self, rhs)
        }
    }
}

impl std::ops::BitOr for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::bitor(self, rhs)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::bitor(self, rhs)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::bitor(self, rhs)
        }
    }
}

impl std::ops::BitXor for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::bitxor(self, rhs)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::bitxor(self, rhs)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::bitxor(self, rhs)
        }
    }
}

impl std::ops::Not for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn not(self) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::not(self)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::not(self)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::not(self)
        }
    }
}

impl std::ops::Shl<u32> for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn shl(self, rhs: u32) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::shl(self, rhs)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::shl(self, rhs)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::shl(self, rhs)
        }
    }
}

impl std::ops::Shr<u32> for SimdBitboard {
    type Output = Self;
    
    #[inline(always)]
    fn shr(self, rhs: u32) -> Self::Output {
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        {
            x86_64_simd::shr(self, rhs)
        }
        #[cfg(all(feature = "simd", target_arch = "aarch64"))]
        {
            aarch64_simd::shr(self, rhs)
        }
        #[cfg(not(all(feature = "simd", any(target_arch = "x86_64", target_arch = "aarch64"))))]
        {
            scalar_fallback::shr(self, rhs)
        }
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
