#[cfg(test)]
mod bench {
    use crate::processing::{calculate_phash, phash_from_img, ultra_fast_phash};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    fn bench_hash_methods() {
        let file_path =
            Path::new("/Users/richardlyon/Desktop/test-images/original_images/2024-10-05-1.jpg");
        let img = image::open(file_path).unwrap();
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
