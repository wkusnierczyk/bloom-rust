use bloom::BloomFilter;

fn main() {
    // Configuration
    let expected_elements = 100_000;

    // You can initialize with a false positive rate
    let fp_rate = 0.01;
    let mut filter = BloomFilter::new(expected_elements, fp_rate);

    // OR initialize using a specific hash count (commented out example)
    // let hashes = 7u32;
    // let mut filter = BloomFilter::new(expected_elements, hashes);

    println!("Initializing Bloom Filter...");
    println!("Expected elements: {}", expected_elements);
    println!("Target False Positive Rate: {}", fp_rate);
    println!("Actual Hash Count (k): {}", filter.hash_count());

    // Insert data
    println!("Inserting values 0 to {}...", expected_elements);
    for i in 0..expected_elements {
        filter.insert(&i);
    }

    // Check true positives
    println!("Checking 10 known values...");
    let mut present_count = 0;
    for i in 0..10 {
        if filter.contains(&i) {
            present_count += 1;
        }
    }
    println!("Found {}/10 known values (Should be 10)", present_count);

    // Check false positives
    // We check numbers outside the inserted range.
    let check_range_start = expected_elements;
    let check_range_end = expected_elements + 10_000;
    println!("Checking range {} to {} for false positives...", check_range_start, check_range_end);

    let mut fp_count = 0;
    for i in check_range_start..check_range_end {
        if filter.contains(&i) {
            fp_count += 1;
        }
    }

    let actual_fp_rate = fp_count as f64 / 10_000.0;
    println!("False Positives found: {}", fp_count);
    println!("Actual FP Rate: {:.4} (Target: {})", actual_fp_rate, actual_fp_rate);
}
