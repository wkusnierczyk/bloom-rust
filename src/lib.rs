use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// Configuration parameter for creating a Bloom Filter.
///
/// Specify either the desired false positive rate (f64)
/// or the specific number of hash functions (u32) to use.
#[derive(Debug, Clone, Copy)]
pub enum FilterParams {
    /// Target false positive rate (between 0.0 and 1.0).
    /// The filter will calculate optimal bits (m) and hashes (k).
    FalsePositiveRate(f64),
    /// Target number of hash functions (k).
    /// The filter will calculate the optimal bits (m) to satisfy the
    /// 50% fill rate assumption for this k (p = 2^-k).
    HashCount(u32),
}

impl From<f64> for FilterParams {
    fn from(rate: f64) -> Self {
        FilterParams::FalsePositiveRate(rate)
    }
}

impl From<u32> for FilterParams {
    fn from(hashes: u32) -> Self {
        FilterParams::HashCount(hashes)
    }
}

/// A space and time efficient Bloom Filter implementation.
///
/// This structure uses a `Vec<u64>` as a bit array for memory efficiency and
/// implements double-hashing to simulate `k` hash functions with only two
/// real hash computations.
///
/// # Type Parameters
/// * `T`: The type of values to be stored. Must implement `Hash`.
#[derive(Debug, Clone)]
pub struct BloomFilter<T: ?Sized> {
    /// The bit array stored as a vector of u64s to maximize cache efficiency.
    bit_vec: Vec<u64>,
    /// The total number of bits in the filter (m).
    bit_count: u64,
    /// The number of hash functions to use (k).
    hash_fn_count: u32,
    /// Phantom data to hold the type information.
    _marker: PhantomData<T>,
}

impl<T: ?Sized + Hash> BloomFilter<T> {
    /// Creates a new Bloom Filter optimized for the given expected item count
    /// and configuration (either false positive rate or hash count).
    ///
    /// # Arguments
    ///
    /// * `expected_items` - The expected number of items to insert (n).
    /// * `params` - Either a `f64` (false positive rate) or `u32` (number of hashes).
    ///
    /// # Examples
    ///
    /// ```
    /// use bloom::BloomFilter;
    ///
    /// // Initialize with a desired false positive rate
    /// let mut bf1: BloomFilter<str> = BloomFilter::new(1000, 0.01);
    ///
    /// // Initialize with a specific hash count
    /// let mut bf2: BloomFilter<str> = BloomFilter::new(1000, 7u32);
    ///
    /// // Insert an item
    /// bf1.insert("seen");
    ///
    /// // Check for an item
    /// _ = bf1.contains("seen");
    /// _ = bf1.contains("unseen");
    ///
    /// // Clear the filter
    /// bf1.clear();
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `expected_items` is 0, or if configuration parameters are invalid
    /// (e.g., rate <= 0.0, rate >= 1.0, or hashes == 0).
    pub fn new(expected_items: usize, params: impl Into<FilterParams>) -> Self {
        assert!(expected_items > 0, "Expected items must be greater than 0.");

        let ln2 = std::f64::consts::LN_2;
        let params = params.into();

        let (m, k) = match params {
            FilterParams::FalsePositiveRate(p) => {
                assert!(
                    p > 0.0 && p < 1.0,
                    "False positive rate must be between 0.0 and 1.0, exclusive."
                );
                // m = - (n * ln(p)) / (ln(2)^2)
                let numerator = -1.0 * (expected_items as f64) * p.ln();
                let denominator = ln2 * ln2;
                let m = (numerator / denominator).ceil() as u64;

                // k = (m / n) * ln(2)
                let k = ((m as f64 / expected_items as f64) * ln2).ceil() as u32;
                (m, k)
            }
            FilterParams::HashCount(k) => {
                assert!(k > 0, "Hash count must be greater than 0.");
                // If k is fixed, assume optimal fill rate (50%), where p = 2^-k.
                // Derived from k = (m/n) * ln(2) -> m = (k * n) / ln(2)
                let m = ((k as f64 * expected_items as f64) / ln2).ceil() as u64;
                (m, k)
            }
        };

        // Round up m to the nearest multiple of 64 for valid u64 storage
        let num_u64s = ((m + 63) / 64) as usize;
        let bit_vec = vec![0; num_u64s];

        // Recalculate true bit count based on vector size
        let true_bit_count = (num_u64s * 64) as u64;

        BloomFilter {
            bit_vec,
            bit_count: true_bit_count,
            hash_fn_count: k,
            _marker: PhantomData,
        }
    }

    /// Inserts an item into the Bloom Filter.
    pub fn insert(&mut self, item: &T) {
        let (h1, h2) = self.get_hashes(item);
        for i in 0..self.hash_fn_count {
            let (vec_index, mask) = self.get_bit(h1, h2, i);
            self.bit_vec[vec_index] |= mask;
        }
    }

    /// Checks if an item might be in the Bloom Filter.
    ///
    /// Returns `true` if the item might be present (with a probability of false positive).
    /// Returns `false` if the item is definitely not present.
    pub fn contains(&self, item: &T) -> bool {
        let (h1, h2) = self.get_hashes(item);
        for i in 0..self.hash_fn_count {
            let (vec_index, mask) = self.get_bit(h1, h2, i);
            if (self.bit_vec[vec_index] & mask) == 0 {
                return false;
            }
        }
        true
    }

    /// Clears all bits in the filter.
    pub fn clear(&mut self) {
        for slot in self.bit_vec.iter_mut() {
            *slot = 0;
        }
    }

    /// Computes two 64-bit hashes for the item.
    fn get_hashes(&self, item: &T) -> (u64, u64) {
        let mut hasher1 = DefaultHasher::new();
        item.hash(&mut hasher1);
        let h1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        item.hash(&mut hasher2);
        h1.hash(&mut hasher2);
        let h2 = hasher2.finish();

        (h1, h2)
    }

    /// Calculates the bit index for the i-th hash function using Double Hashing.
    #[inline]
    fn get_index(&self, h1: u64, h2: u64, i: u32) -> u64 {
        let offset = h2.wrapping_mul(i as u64);
        let hash = h1.wrapping_add(offset);

        hash % self.bit_count
    }

    /// Computes the vector index and bit mask for the i-th hash position.
    #[inline]
    fn get_bit(&self, h1: u64, h2: u64, i: u32) -> (usize, u64) {
        let bit_index = self.get_index(h1, h2, i);
        let vec_index = (bit_index / 64) as usize;
        let bit_offset = 1u64 << (bit_index % 64);

        (vec_index, bit_offset)
    }

    /// Returns the approximate memory usage of the bit vector in bytes.
    pub fn memory_usage_bytes(&self) -> usize {
        self.bit_vec.capacity() * 8
    }

    /// Returns the number of hash functions (k) being used.
    pub fn hash_count(&self) -> u32 {
        self.hash_fn_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_with_rate() {
        let bf: BloomFilter<str> = BloomFilter::new(100, 0.01);
        assert_eq!(bf.bit_vec.len(), 15);
        assert!(bf.hash_fn_count > 0);
    }

    #[test]
    fn test_initialization_with_hashes() {
        // k=7 implies p ~= 0.01. m should be roughly the same as above.
        let bf: BloomFilter<str> = BloomFilter::new(100, 7u32);
        assert_eq!(bf.hash_fn_count, 7);
        // Calculated m should be sufficient for 100 items with k=7
        // m = k * n / ln(2) = 700 / 0.693 = 1010 bits -> ~16 u64s
        assert!(bf.bit_vec.len() >= 15);
    }

    #[test]
    fn test_insert_and_contains() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert("seen");
        bf.insert("also seen");

        assert!(bf.contains("seen"));
        assert!(bf.contains("also seen"));
        assert!(!bf.contains("unseen"));
    }

    #[test]
    fn test_clear() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert(&1);
        bf.clear();
        assert!(!bf.contains(&1));
    }

    #[test]
    fn test_custom_struct_support() {
        #[derive(Hash)]
        struct User {
            id: u32,
            name: String,
        }

        let mut bf = BloomFilter::new(10, 0.01);
        let user = User {
            id: 1,
            name: "Alyssa P. Hacker".to_string(),
        };

        bf.insert(&user);
        assert!(bf.contains(&user));

        let other_user = User {
            id: 2,
            name: "Eva Lu Ator".to_string(),
        };
        assert!(!bf.contains(&other_user));
    }

    #[test]
    #[should_panic(expected = "Expected items must be greater than 0.")]
    fn test_panic_on_zero_items() {
        BloomFilter::<i32>::new(0, 0.01);
    }

    #[test]
    #[should_panic(expected = "Hash count must be greater than 0.")]
    fn test_panic_on_zero_hashes() {
        BloomFilter::<i32>::new(100, 0u32);
    }
}
