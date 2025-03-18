use image_deduper_core::processing::{metal_phash, perceptual};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    println!("Testing Metal GPU acceleration on large image batches");

    // Find test images
    let test_dirs = [
        Path::new("../test_data/test_images"),
        Path::new("test_data/test_images"),
    ];

    let mut images = Vec::new();

    // Try to find test images
    for dir in &test_dirs {
        if dir.exists() {
            println!("Found test directory: {}", dir.display());

            // Find all image files in subdirectories (limit to 50)
            find_image_files(dir, &mut images, 50);

            if !images.is_empty() {
                break;
            }
        }
    }

    if images.is_empty() {
        println!("No test images found. Please check that test_images directory exists.");
        return;
    }

    println!("Found {} test images for benchmarking", images.len());
    run_batch_test(&images);
}

// Find all image files in a directory and its subdirectories
fn find_image_files(dir: &Path, images: &mut Vec<PathBuf>, limit: usize) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if images.len() >= limit {
                return;
            }

            if let Ok(entry) = entry {
                let path = entry.path();

                if path.is_dir() {
                    find_image_files(&path, images, limit);
                } else if is_image_file(&path) {
                    images.push(path);
                }
            }
        }
    }
}

// Check if a file is likely an image
fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        match ext.to_string_lossy().to_lowercase().as_str() {
            "jpg" | "jpeg" | "png" | "tiff" | "tif" => true,
            _ => false,
        }
    } else {
        false
    }
}

// Run the batch test comparing CPU and GPU performance
fn run_batch_test(images: &[PathBuf]) {
    println!("\nLoading and processing {} images", images.len());

    // First load all images to memory to avoid I/O overhead in the timing tests
    let mut loaded_images = Vec::new();

    for path in images {
        match image::open(path) {
            Ok(img) => loaded_images.push(img),
            Err(e) => println!("Error loading {}: {}", path.display(), e),
        }
    }

    println!("Successfully loaded {} images", loaded_images.len());

    // First test CPU performance
    println!("\nBenchmarking CPU processing...");
    let cpu_start = Instant::now();
    let mut cpu_hashes = Vec::new();

    for img in &loaded_images {
        cpu_hashes.push(perceptual::calculate_phash(img));
    }

    let cpu_duration = cpu_start.elapsed();
    let cpu_avg = cpu_duration.as_micros() as f64 / loaded_images.len() as f64;

    println!(
        "CPU processed {} images in {:?}",
        loaded_images.len(),
        cpu_duration
    );
    println!("CPU average: {:.2}µs per image", cpu_avg);

    // Now test GPU performance if available
    if let Some(_) = metal_phash::metal_phash(&loaded_images[0]) {
        println!("\nBenchmarking GPU processing...");
        let gpu_start = Instant::now();
        let mut gpu_hashes = Vec::new();

        for img in &loaded_images {
            if let Some(hash) = metal_phash::metal_phash(img) {
                gpu_hashes.push(hash);
            }
        }

        let gpu_duration = gpu_start.elapsed();
        let gpu_avg = gpu_duration.as_micros() as f64 / loaded_images.len() as f64;

        println!(
            "GPU processed {} images in {:?}",
            gpu_hashes.len(),
            gpu_duration
        );
        println!("GPU average: {:.2}µs per image", gpu_avg);

        // Compare results
        println!("\nPerformance comparison:");
        let speedup = cpu_avg / gpu_avg;
        println!("GPU speedup: {:.2}x", speedup);

        // Compare hash quality
        let mut match_count = 0;
        let mut total_distance = 0;

        for i in 0..gpu_hashes.len().min(cpu_hashes.len()) {
            let distance = match (&gpu_hashes[i], &cpu_hashes[i]) {
                (perceptual::PHash::Standard(a), perceptual::PHash::Standard(b)) => {
                    (a ^ b).count_ones()
                }
                _ => 0, // Handle other cases if needed
            };
            total_distance += distance;

            if distance <= 3 {
                match_count += 1;
            }
        }

        let avg_distance = total_distance as f64 / gpu_hashes.len().min(cpu_hashes.len()) as f64;
        let match_percent =
            (match_count as f64 * 100.0) / gpu_hashes.len().min(cpu_hashes.len()) as f64;

        println!("Average Hamming distance: {:.2} bits", avg_distance);
        println!("Close matches (≤3 bits different): {:.1}%", match_percent);

        // Run more extensive testing with different image sizes
        println!("\nTesting GPU performance across different image sizes:");

        // Create a series of test images at different sizes
        let test_sizes = [(512, 512), (1024, 1024), (2048, 2048), (4096, 4096)];

        // Test performance at each size
        for &(width, height) in &test_sizes {
            // Create a test image of the specified size
            let mut test_image = image::RgbImage::new(width, height);

            // Fill with random data
            for y in 0..height {
                for x in 0..width {
                    let r = (x % 255) as u8;
                    let g = (y % 255) as u8;
                    let b = ((x + y) % 255) as u8;
                    test_image.put_pixel(x, y, image::Rgb([r, g, b]));
                }
            }

            let img = image::DynamicImage::ImageRgb8(test_image);

            // Warm up
            let _ = perceptual::calculate_phash(&img);
            let _ = metal_phash::metal_phash(&img);

            // Run multiple iterations for more accurate timing
            const ITERATIONS: u32 = 5;

            // CPU timing
            let cpu_start = Instant::now();
            let mut cpu_hash = perceptual::PHash::Standard(0);
            for _ in 0..ITERATIONS {
                cpu_hash = perceptual::calculate_phash(&img);
            }
            let cpu_time = cpu_start.elapsed() / ITERATIONS;

            // GPU timing
            let gpu_start = Instant::now();
            let mut gpu_hash = None;
            for _ in 0..ITERATIONS {
                gpu_hash = metal_phash::metal_phash(&img);
            }
            let gpu_time = gpu_start.elapsed() / ITERATIONS;

            if let Some(hash) = gpu_hash {
                let distance = match (&cpu_hash, &hash) {
                    (perceptual::PHash::Standard(a), perceptual::PHash::Standard(b)) => {
                        (a ^ b).count_ones()
                    }
                    _ => 0, // Handle other cases if needed
                };
                let speedup = cpu_time.as_nanos() as f64 / gpu_time.as_nanos() as f64;

                println!(
                    "Image {}x{}: CPU={:?}, GPU={:?}, Speedup={:.2}x, Hamming distance={}",
                    width, height, cpu_time, gpu_time, speedup, distance
                );
            } else {
                println!(
                    "Image {}x{}: CPU={:?}, GPU=Not available",
                    width, height, cpu_time
                );
            }

            // For very large images, also test batches of 10 to see parallel performance
            if width >= 2048 && height >= 2048 {
                println!(
                    "  Testing batch processing for {}x{} images...",
                    width, height
                );

                // Create 10 similar images (this simulates a real batch)
                let batch: Vec<_> = (0..10)
                    .map(|i| {
                        let mut test_image = image::RgbImage::new(width, height);

                        // Fill with slightly different random data
                        for y in 0..height {
                            for x in 0..width {
                                let r = ((x + i) % 255) as u8;
                                let g = ((y + i) % 255) as u8;
                                let b = ((x + y + i) % 255) as u8;
                                test_image.put_pixel(x, y, image::Rgb([r, g, b]));
                            }
                        }

                        image::DynamicImage::ImageRgb8(test_image)
                    })
                    .collect();

                // Warm up
                for img in &batch {
                    let _ = perceptual::calculate_phash(img);
                    let _ = metal_phash::metal_phash(img);
                }

                // CPU batch timing - sequential
                let cpu_start_seq = Instant::now();
                for img in &batch {
                    let _ = perceptual::calculate_phash(img);
                }
                let cpu_time_seq = cpu_start_seq.elapsed();

                // CPU batch timing - parallel
                let cpu_start_par = Instant::now();
                batch.par_iter().for_each(|img| {
                    let _ = perceptual::calculate_phash(img);
                });
                let cpu_time_par = cpu_start_par.elapsed();

                // GPU batch timing
                let gpu_start = Instant::now();
                for img in &batch {
                    let _ = metal_phash::metal_phash(img);
                }
                let gpu_time = gpu_start.elapsed();

                let speedup_seq = cpu_time_seq.as_nanos() as f64 / gpu_time.as_nanos() as f64;
                let speedup_par = cpu_time_par.as_nanos() as f64 / gpu_time.as_nanos() as f64;

                println!("  Batch of 10 {}x{} images:", width, height);
                println!("    CPU sequential: {:?}", cpu_time_seq);
                println!("    CPU parallel:   {:?}", cpu_time_par);
                println!("    GPU sequential: {:?}", gpu_time);
                println!("    GPU vs CPU seq: {:.2}x speedup", speedup_seq);
                println!("    GPU vs CPU par: {:.2}x speedup", speedup_par);
            }
        }
    } else {
        println!("Metal GPU acceleration not available on this system");
    }
}
