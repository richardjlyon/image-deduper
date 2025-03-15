#[cfg(test)]
mod bench {
    use crate::processing::{calculate_phash, phash_from_img, ultra_fast_phash};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    // Use a direct function rather than trying to load the module
    fn get_test_image() -> image::DynamicImage {
        // This implementation is the same as in tests/common/test_images.rs
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/test_images/jpg/IMG-2624x3636/IMG-2624x3636_original.jpg");
        image::open(&path).expect("Failed to open test image")
    }

    fn bench_hash_methods() {
        // Use the test image function
        let img = get_test_image();

        println!("Image dimensions: {}x{}", img.width(), img.height());

        // Benchmark original method
        let mut total_duration = Duration::from_secs(0);
        for _ in 0..5 {
            let start = Instant::now();
            let _phash = phash_from_img(&img);
            let duration = start.elapsed();
            total_duration += duration;
        }
        let average_duration = total_duration / 5;
        println!("Original method average time: {:?}", average_duration);

        // Benchmark standard optimized method
        let mut total_duration = Duration::from_secs(0);
        for _ in 0..5 {
            let start = Instant::now();
            let _phash = calculate_phash(&img);
            let duration = start.elapsed();
            total_duration += duration;
        }
        let average_duration = total_duration / 5;
        println!("Optimized method average time: {:?}", average_duration);

        // Benchmark ultra fast method
        let mut total_duration = Duration::from_secs(0);
        for _ in 0..5 {
            let start = Instant::now();
            let _phash = ultra_fast_phash(&img);
            let duration = start.elapsed();
            total_duration += duration;
        }
        let average_duration = total_duration / 5;
        println!("Ultra-fast method average time: {:?}", average_duration);

        // Compare the hashes to see if they're similar
        let original_hash = phash_from_img(&img);
        let optimized_hash = calculate_phash(&img);
        let ultra_fast_hash = ultra_fast_phash(&img);

        println!("\nHash comparison:");
        println!(
            "Distance between original and optimized: {}",
            original_hash.distance(&optimized_hash)
        );
        println!(
            "Distance between original and ultra-fast: {}",
            original_hash.distance(&ultra_fast_hash)
        );
        println!(
            "Distance between optimized and ultra-fast: {}",
            optimized_hash.distance(&ultra_fast_hash)
        );
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn run_hash_benchmark() {
            bench_hash_methods();
        }
    }
}
