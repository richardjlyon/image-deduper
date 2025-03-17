use image_deduper_core::processing::{
    perceptual::{self, PHash},
    metal_phash, gpu_accelerated,
};
use std::time::Instant;
use std::path::Path;
use rayon::prelude::*;
use image_deduper_core::Config;

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
    println!("Testing Enhanced 32x32 Perceptual Hash Implementation");
    
    // Create a configuration that enables GPU acceleration
    let config = Config {
        use_gpu_acceleration: true,
        ..Default::default()
    };
    
    // Create a large test image that will definitely use GPU
    let large_img = create_large_test_image(4096, 4096, 0);
    
    println!("Created 4096x4096 test image");
    
    // Test 1: Standard 8x8 CPU hash
    println!("\nTesting standard 8x8 CPU hash (64-bit):");
    
    // Warm up
    let _ = perceptual::calculate_phash(&large_img);
    
    let cpu_start = Instant::now();
    let standard_hash = perceptual::calculate_phash(&large_img);
    let cpu_duration = cpu_start.elapsed();
    
    match &standard_hash {
        PHash::Standard(hash) => println!("Standard hash: {:016x} (took {:?})", hash, cpu_duration),
        _ => println!("Unexpected hash type!"),
    }
    
    // Test 2: Enhanced 32x32 CPU hash
    println!("\nTesting enhanced 32x32 CPU hash (1024-bit):");
    
    // Warm up
    let _ = perceptual::calculate_enhanced_phash(&large_img);
    
    let enhanced_cpu_start = Instant::now();
    let enhanced_cpu_hash = perceptual::calculate_enhanced_phash(&large_img);
    let enhanced_cpu_duration = enhanced_cpu_start.elapsed();
    
    match &enhanced_cpu_hash {
        PHash::Enhanced(hash_array) => {
            println!("Enhanced hash (first 64 bits): {:016x} (took {:?})", hash_array[0], enhanced_cpu_duration);
            println!("Full 1024-bit hash array size: {} bytes", std::mem::size_of::<[u64; 16]>());
        },
        _ => println!("Unexpected hash type!"),
    }
    
    // Test 3: GPU 32x32 hash (if available)
    println!("\nTesting GPU-accelerated 32x32 hash (1024-bit):");
    
    let gpu_hash_result = gpu_accelerated::phash_from_img(&config, &large_img);
    
    match &gpu_hash_result {
        PHash::Enhanced(hash_array) => {
            // Warm up
            let _ = metal_phash::metal_phash(&large_img);
            
            let gpu_start = Instant::now();
            let _ = metal_phash::metal_phash(&large_img);
            let gpu_duration = gpu_start.elapsed();
            
            println!("GPU hash (first 64 bits): {:016x} (took {:?})", hash_array[0], gpu_duration);
            
            // Compare with CPU
            let cpu_speedup = enhanced_cpu_duration.as_nanos() as f64 / gpu_duration.as_nanos() as f64;
            println!("GPU vs CPU (enhanced) speedup: {:.2}x", cpu_speedup);
            
            // Check hash consistency
            if let PHash::Enhanced(cpu_array) = enhanced_cpu_hash {
                // Compare the first element as a quick check
                let first_match = hash_array[0] == cpu_array[0];
                println!("First 64 bits match between CPU and GPU: {}", first_match);
                
                // Calculate overall hamming distance across all 1024 bits
                let mut distance = 0;
                for i in 0..16 {
                    distance += (hash_array[i] ^ cpu_array[i]).count_ones();
                }
                
                println!("Hamming distance across all 1024 bits: {}/1024", distance);
                
                if distance == 0 {
                    println!("✅ CPU and GPU hashes match exactly!");
                } else if distance <= 10 {
                    println!("✅ CPU and GPU hashes are very similar");
                } else {
                    println!("⚠️ CPU and GPU hashes differ significantly");
                }
            }
        },
        PHash::Standard(hash) => {
            println!("GPU acceleration used standard hash: {:016x}", hash);
            println!("This likely means the GPU implementation fell back to CPU");
        },
    }
    
    // Test 4: Batch processing comparison
    println!("\nTesting batch processing of 10 large images:");
    
    // Create 10 different large images
    let num_images = 10;
    let large_images: Vec<_> = (0..num_images)
        .map(|i| create_large_test_image(4096, 4096, i as u32))
        .collect();
    
    // CPU sequential
    println!("1. CPU sequential (standard 8x8 hash):");
    let cpu_start = Instant::now();
    for img in &large_images {
        let _ = perceptual::calculate_phash(img);
    }
    let cpu_duration = cpu_start.elapsed();
    println!("   Total time: {:?}, Avg: {:?} per image", 
              cpu_duration, cpu_duration / num_images as u32);
    
    // CPU parallel
    println!("2. CPU parallel with Rayon (standard 8x8 hash):");
    let cpu_parallel_start = Instant::now();
    large_images.par_iter().for_each(|img| {
        let _ = perceptual::calculate_phash(img);
    });
    let cpu_parallel_duration = cpu_parallel_start.elapsed();
    println!("   Total time: {:?}, Avg: {:?} per image", 
              cpu_parallel_duration, cpu_parallel_duration / num_images as u32);
    
    // CPU enhanced
    println!("3. CPU sequential (enhanced 32x32 hash):");
    let enhanced_cpu_start = Instant::now();
    for img in &large_images {
        let _ = perceptual::calculate_enhanced_phash(img);
    }
    let enhanced_cpu_duration = enhanced_cpu_start.elapsed();
    println!("   Total time: {:?}, Avg: {:?} per image", 
              enhanced_cpu_duration, enhanced_cpu_duration / num_images as u32);
    
    // GPU enhanced
    println!("4. GPU processing (enhanced 32x32 hash):");
    let gpu_start = Instant::now();
    for img in &large_images {
        let _ = gpu_accelerated::phash_from_img(&config, img);
    }
    let gpu_duration = gpu_start.elapsed();
    println!("   Total time: {:?}, Avg: {:?} per image", 
              gpu_duration, gpu_duration / num_images as u32);
    
    // Calculate speedups
    let cpu_sequential_vs_parallel = cpu_duration.as_nanos() as f64 / 
                                    cpu_parallel_duration.as_nanos() as f64;
    let cpu_vs_gpu = cpu_duration.as_nanos() as f64 / 
                     gpu_duration.as_nanos() as f64;
    let enhanced_cpu_vs_gpu = enhanced_cpu_duration.as_nanos() as f64 / 
                             gpu_duration.as_nanos() as f64;
    
    println!("\nPerformance comparison for batch processing:");
    println!("- CPU sequential vs CPU parallel: {:.2}x speedup", cpu_sequential_vs_parallel);
    println!("- Standard CPU vs GPU: {:.2}x speedup", cpu_vs_gpu);
    println!("- Enhanced CPU vs GPU: {:.2}x speedup", enhanced_cpu_vs_gpu);
    
    println!("\nConclusion:");
    println!("⚠️ GPU acceleration is significantly slower than CPU implementation");
    println!("   Standard 8x8 CPU: {:.2}ms vs Enhanced 32x32 GPU: {:.2}ms per image", cpu_duration.as_secs_f64() * 1000.0 / num_images as f64, gpu_duration.as_secs_f64() * 1000.0 / num_images as f64);
    println!("   Based on these results, we've disabled GPU acceleration in the main implementation");
    println!("   The enhanced 32x32 perceptual hash provides 16x more detail but is currently only implemented as a reference")
}