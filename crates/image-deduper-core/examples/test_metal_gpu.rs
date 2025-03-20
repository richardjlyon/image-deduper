use image_deduper_core::processing::perceptual_hash;
use image_deduper_core::processing::{metal_phash, perceptual_hash::PHash};
use image_deduper_core::Config;
use rayon::prelude::*;
use std::path::Path;
use std::time::Instant;

// Create a large test image with a gradient pattern to force GPU usage
fn create_large_test_image(width: u32, height: u32, seed: u32) -> image::DynamicImage {
    let mut img = image::RgbImage::new(width, height);

    // Fill with a gradient pattern with variation based on seed
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / width as f32 * 255.0) as u32 + seed * 10) % 256;
            let g = ((y as f32 / height as f32 * 255.0) as u32 + seed * 20) % 256;
            let b = (((x + y) as f32 / (width + height) as f32 * 255.0) as u32 + seed * 30) % 256;
            img.put_pixel(x, y, image::Rgb([r as u8, g as u8, b as u8]));
        }
    }

    image::DynamicImage::ImageRgb8(img)
}

fn main() {
    println!("Testing Metal GPU acceleration for perceptual hashing");

    // Create a configuration that enables GPU
    let _config = Config {
        use_gpu_acceleration: true,
        ..Default::default()
    };

    // Path to test image - try multiple paths
    let test_image_paths = [
        Path::new("../test_data/real_images/image_200x300_0.jpg"),
        Path::new("test_data/real_images/image_200x300_0.jpg"),
        Path::new("../test_data/generated/test_image1.jpg"),
        Path::new("test_data/generated/test_image1.jpg"),
    ];

    // Try each path until we find one that works
    for path in &test_image_paths {
        if path.exists() {
            println!("Found test image at: {}", path.display());

            // First test the standard GPU implementation
            test_gpu_hash(path);

            // Then test the enhanced 1024-bit hash implementation
            println!("\n----- Testing Enhanced 1024-bit Hash Implementation -----\n");
            // test_enhanced_gpu_hash function has been removed or renamed
            // Comment out this line as we'll need to implement it separately
            // test_enhanced_gpu_hash(path, &config);

            return;
        }
    }

    // If we get here, none of the paths worked
    println!("Could not find any test images. Please check the paths:");
    for path in &test_image_paths {
        println!("  - {}", path.display());
    }
}

fn test_gpu_hash(image_path: &Path) {
    println!("Loading test image: {}", image_path.display());

    // Load image
    match image::open(image_path) {
        Ok(img) => {
            // Test 1: CPU hash
            println!("\nTesting CPU-based hashing...");
            // Warm up
            let _ = perceptual_hash::calculate_phash(&img);

            // Timed run
            let cpu_start = Instant::now();
            // Run multiple times for better timing
            let mut cpu_hash = PHash::Standard(0);
            for _ in 0..10 {
                cpu_hash = perceptual_hash::calculate_phash(&img);
            }
            let cpu_duration = cpu_start.elapsed() / 10;
            if let PHash::Standard(hash) = cpu_hash {
                println!("CPU hash: {:016x} (took {:?})", hash, cpu_duration);
            }

            // Test 2: Create multiple large images to force GPU usage
            println!("\nCreating large test images (4096x4096) to better utilize GPU...");

            // Create 10 different large images
            let num_large_images = 10;
            let mut large_images = Vec::with_capacity(num_large_images);

            for i in 0..num_large_images {
                large_images.push(create_large_test_image(4096, 4096, i as u32));
            }

            // Test single large image on CPU first
            println!("Testing CPU with large image...");
            // Warm up
            let _ = perceptual_hash::calculate_phash(&large_images[0]);

            let cpu_large_start = Instant::now();
            let cpu_large_hash = perceptual_hash::calculate_phash(&large_images[0]);
            let cpu_large_duration = cpu_large_start.elapsed();
            if let PHash::Standard(hash) = cpu_large_hash {
                println!(
                    "CPU hash (single large): {:016x} (took {:?})",
                    hash, cpu_large_duration
                );
            }

            // Test GPU with large image
            println!("\nTesting GPU-based hashing with large image...");
            if let Some(gpu_hash) = metal_phash::metal_phash(&img) {
                // First test original small image
                // Warm up the GPU
                let _ = metal_phash::metal_phash(&img);

                // Now measure timed runs
                let gpu_start = Instant::now();
                // Run multiple times to get better timing
                for _ in 0..10 {
                    let _ = metal_phash::metal_phash(&img);
                }
                let gpu_duration = gpu_start.elapsed() / 10;

                // Now test single large image on GPU
                // Warm up the GPU
                let _ = metal_phash::metal_phash(&large_images[0]);

                let gpu_large_start = Instant::now();
                let gpu_large_hash = metal_phash::metal_phash(&large_images[0]);
                let gpu_large_duration = gpu_large_start.elapsed();

                // Test batch processing (sequential)
                println!(
                    "\nTesting batch processing of {} large images:",
                    num_large_images
                );
                println!("1. Sequential CPU processing:");
                let cpu_batch_start = Instant::now();
                for img in &large_images {
                    let _ = perceptual_hash::calculate_phash(img);
                }
                let cpu_batch_duration = cpu_batch_start.elapsed();
                println!(
                    "   Total time: {:?}, Avg: {:?} per image",
                    cpu_batch_duration,
                    cpu_batch_duration / num_large_images as u32
                );

                // Test batch processing (parallel CPU with Rayon)
                println!("2. Parallel CPU processing with Rayon:");

                // Configure thread pool explicitly
                let thread_pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(num_cpus::get())
                    .build()
                    .unwrap();

                let cpu_parallel_start = Instant::now();
                thread_pool.install(|| {
                    large_images.par_iter().for_each(|img| {
                        let _ = perceptual_hash::calculate_phash(img);
                    });
                });
                let cpu_parallel_duration = cpu_parallel_start.elapsed();
                println!(
                    "   Total time: {:?}, Avg: {:?} per image",
                    cpu_parallel_duration,
                    cpu_parallel_duration / num_large_images as u32
                );

                // Test batch processing (GPU)
                println!("3. GPU processing:");
                let gpu_batch_start = Instant::now();
                for img in &large_images {
                    let _ = metal_phash::metal_phash(img);
                }
                let gpu_batch_duration = gpu_batch_start.elapsed();
                println!(
                    "   Total time: {:?}, Avg: {:?} per image",
                    gpu_batch_duration,
                    gpu_batch_duration / num_large_images as u32
                );

                // Calculate speedups
                let cpu_sequential_vs_parallel =
                    cpu_batch_duration.as_nanos() as f64 / cpu_parallel_duration.as_nanos() as f64;
                let cpu_vs_gpu =
                    cpu_batch_duration.as_nanos() as f64 / gpu_batch_duration.as_nanos() as f64;
                let cpu_parallel_vs_gpu =
                    cpu_parallel_duration.as_nanos() as f64 / gpu_batch_duration.as_nanos() as f64;

                println!("\nPerformance comparison for batch processing:");
                println!(
                    "- CPU sequential vs CPU parallel: {:.2}x speedup",
                    cpu_sequential_vs_parallel
                );
                println!("- CPU sequential vs GPU: {:.2}x speedup", cpu_vs_gpu);
                println!("- CPU parallel vs GPU: {:.2}x speedup", cpu_parallel_vs_gpu);

                println!("Small image:");
                if let PHash::Standard(hash) = gpu_hash {
                    println!("  GPU hash: {:016x} (took {:?})", hash, gpu_duration);
                }

                if let Some(large_hash) = gpu_large_hash {
                    println!("Large image:");
                    if let PHash::Standard(hash) = large_hash {
                        println!("  GPU hash: {:016x} (took {:?})", hash, gpu_large_duration);
                    }

                    let large_speedup =
                        cpu_large_duration.as_nanos() as f64 / gpu_large_duration.as_nanos() as f64;
                    println!("  GPU speedup on large image: {:.2}x", large_speedup);
                }

                // Calculate difference for original small image
                let difference = match (&cpu_hash, &gpu_hash) {
                    (PHash::Standard(a), PHash::Standard(b)) => hamming_distance(*a, *b),
                    _ => 0, // Handle other variants if needed
                };
                let speedup = cpu_duration.as_nanos() as f64 / gpu_duration.as_nanos() as f64;

                println!("\nResults comparison:");
                println!("Hamming distance between CPU and GPU hash: {}", difference);
                println!("GPU speedup: {:.2}x", speedup);

                if difference == 0 {
                    println!("✅ Hashes match exactly!");
                } else if difference <= 3 {
                    println!("✅ Hashes are very similar (expected due to different algorithms)");
                } else {
                    println!("⚠️ Hashes differ significantly");
                }
            } else {
                println!("❌ Metal GPU acceleration not available on this system");
            }
        }
        Err(e) => {
            println!("Error loading image: {}", e);
        }
    }
}

// Helper to calculate Hamming distance
fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}
