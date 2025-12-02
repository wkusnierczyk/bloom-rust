use bloomlib::BloomFilter;
use std::time::{Duration, Instant};

/// A simple Xorshift pseudo-random number generator.
/// Used to generate deterministic test data without external dependencies.
/// Also see https://docs.rs/xorshift/latest/xorshift
struct Random {
    state: u64,
}

impl Random {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut state = self.state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.state = state;
        state
    }
}

fn main() {
    // Run a few benchmark configurations
    run_benchmark(1_000_000, 0.000_1);
    run_benchmark(1_000_000, 0.000_000_000_1);
    run_benchmark(100_000_000, 0.000_000_000_000_1);
}

fn run_benchmark(n: usize, fp_rate: f64) {
    // --- Setup ---
    let mut bf = BloomFilter::new(n, fp_rate);
    let mut rng = Random::new(12345);

    println!("--- Bloom Filter Performance Benchmark ---");
    println!("Items:          {}", n);
    println!("Target FP Rate: {}", fp_rate);
    println!("Hash Count:     {}", bf.hash_count());
    println!("------------------------------------------\n");

    // Generate data
    let mut dataset = Vec::with_capacity(n);
    for _ in 0..n {
        dataset.push(rng.next_u64());
    }

    // --- Metric 1: Memory Usage ---
    let ki = 1024.0;
    let memory_bytes = bf.memory_usage_bytes();
    let memory_mb = memory_bytes as f64 / ki / ki;
    println!("\n[Memory Usage]");
    println!(
        "Bit Vector Size: {:.2} MB ({} bytes)",
        memory_mb, memory_bytes
    );
    println!(
        "Bits per item: {:.2} bits",
        (memory_bytes * 8) as f64 / n as f64
    );

    // --- Metric 2: Insert Performance ---
    println!("\n[Insertion Performance]");
    let start_insert = Instant::now();
    for item in &dataset {
        bf.insert(item);
    }
    let duration_insert = start_insert.elapsed();
    print_timing("Insert", duration_insert, n);

    // --- Metric 3: Worst Case Lookup (Seen Items) ---
    // The worst case for Bloom Filter is when the item has been seen before.
    // The algorithm MUST compute all k hashes and check k bits.
    println!("\n[Lookup Performance - Worst Case (Seen Items)]");
    let start_worst = Instant::now();
    for item in &dataset {
        // Assign the result to a volatile variable to prevent compiler optimization
        let _ = std::hint::black_box(bf.contains(item));
    }
    let duration_worst = start_worst.elapsed();
    print_timing("Contains (Seen)", duration_worst, n);

    // --- Metric 4: Average Case Lookup (Unseen Items) ---
    // In a well-tuned Bloom Filter (50% bits set), an unseen item
    // often fails on the 1st or 2nd bit check.
    println!("\n[Lookup Performance - Average Case (Unseen Items)]");

    // Generate new random items that are likely NOT in the set
    let mut unseen_dataset = Vec::with_capacity(n);
    for _ in 0..n {
        unseen_dataset.push(rng.next_u64());
    }

    let start_avg = Instant::now();
    for item in &unseen_dataset {
        let _ = std::hint::black_box(bf.contains(item));
    }
    let duration_avg = start_avg.elapsed();
    print_timing("Contains (Unseen)", duration_avg, n);

    // --- Metric 5: Best Case Lookup (Empty Filter) ---
    // The best case is checking against an empty filter; fails on the 1st bit.
    println!("\n[Lookup Performance - Best Case (Empty Filter)]");
    let empty_bf: BloomFilter<u64> = BloomFilter::new(n, fp_rate);

    let start_best = Instant::now();
    for item in &dataset {
        let _ = std::hint::black_box(empty_bf.contains(item));
    }
    let duration_best = start_best.elapsed();
    print_timing("Contains (Empty)", duration_best, n);

    println!("\n==========================================\n");
}

fn print_timing(label: &str, total_duration: Duration, iterations: usize) {
    let total_ns = total_duration.as_nanos() as f64;
    let ns_per_op = total_ns / iterations as f64;
    let ops_per_sec = 1_000_000_000.0 / ns_per_op;

    println!(
        "{:<20}: {:.2} ns/op | {:.2} million ops/sec",
        label,
        ns_per_op,
        ops_per_sec / 1_000_000.0
    );
}
