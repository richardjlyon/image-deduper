/// Benchmark the performance of the hash functions

#[cfg(test)]
mod bench {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    use crate::processing::phash_from_img;
    fn bench_phash() {
        let file_path =
            Path::new("/Users/richardlyon/Desktop/test-images/original_images/IMG_0009.JPG");
        let img = image::open(file_path).unwrap();

        // run this three times and average the results
        let mut total_duration = Duration::from_secs(0);
        for _ in 0..3 {
            let start = Instant::now();
            let _phash = phash_from_img(&img).unwrap();
            let duration = start.elapsed();
            total_duration += duration;
        }
        let average_duration = total_duration / 3;
        println!("Average time taken: {:?}", average_duration);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn run_py_hash_benchmark() {
            bench_phash();
        }
    }
}
