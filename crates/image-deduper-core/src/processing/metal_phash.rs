//! GPU-accelerated perceptual hashing using Metal on macOS
//!
//! This module provides Metal-accelerated implementations of
//! perceptual hash algorithms for image comparison. It achieves
//! significant performance improvements over CPU-based methods.

use crate::processing::perceptual::PHash;
use image::{DynamicImage, GenericImageView};
use metal::{Device, MTLResourceOptions, MTLSize};
use objc::rc::autoreleasepool;
use std::path::Path;
use std::sync::Once;

// Metal shader for calculating grayscale and generating perceptual hash
static METAL_SHADER_SRC: &str = r#"
#include <metal_stdlib>
using namespace metal;

// Compute grayscale and generating perceptual hash with enhanced 32x32 grid
kernel void calculate_phash(
    texture2d<float, access::read> input [[texture(0)]],
    device ulong* result [[buffer(0)]],
    uint2 grid_size [[threads_per_grid]],
    uint2 thread_position_in_grid [[thread_position_in_grid]])
{
    // Multi-threaded version - multiple threads process the image together
    uint thread_index = thread_position_in_grid.y * grid_size.x + thread_position_in_grid.x;
    
    // We use 16 threads to process different parts of the 32x32 grid
    if (thread_index >= 16)
        return;
        
    // Get input dimensions for downsampling
    uint width = input.get_width();
    uint height = input.get_height();
    
    // Create a 32x32 grid of grayscale values for enhanced hashing
    // Each thread works on 1/16th of the grid (64 pixels)
    float gray_pixels[64];
    
    // Figure out which part of the 32x32 grid this thread is responsible for
    uint start_idx = thread_index * 64;
    uint end_idx = start_idx + 64;
    
    // Process 64 pixels for this thread
    for (uint i = 0; i < 64; i++) {
        uint pixel_idx = start_idx + i;
        uint grid_x = pixel_idx % 32;
        uint grid_y = pixel_idx / 32;
        
        // We're filling a 32x32 grid - skip pixels beyond our bounds
        if (grid_y >= 32) continue;
        
        // Calculate region to sample using box filtering
        uint start_x = (grid_x * width) / 32;
        uint end_x = ((grid_x + 1) * width) / 32;
        uint start_y = (grid_y * height) / 32;
        uint end_y = ((grid_y + 1) * height) / 32;
        
        // Calculate step sizes for efficient sampling
        uint step_x = max(1u, (end_x - start_x) / 2);
        uint step_y = max(1u, (end_y - start_y) / 2);
        
        // Sample pixels at regular intervals
        float sum_gray = 0.0;
        uint count = 0;
        
        for (uint py = start_y; py < end_y; py += step_y) {
            for (uint px = start_x; px < end_x; px += step_x) {
                // Read pixel
                float4 pixel = input.read(uint2(min(px, width-1), min(py, height-1)));
                
                // Convert to grayscale using exact same weights as CPU (0.299R + 0.587G + 0.114B)
                sum_gray += 0.299 * pixel.r + 0.587 * pixel.g + 0.114 * pixel.b;
                count++;
            }
        }
        
        // Average and store
        float gray = (count > 0) ? (sum_gray / float(count)) : 0.0;
        gray_pixels[i] = gray;
    }
    
    // Calculate mean of the 64 grayscale values for this thread's part
    float sum = 0.0;
    for (uint i = 0; i < 64; i++) {
        sum += gray_pixels[i];
    }
    float local_mean = sum / 64.0;
    
    // Build hash for this thread's part by comparing each value to mean
    ulong hash = 0;
    
    // Each bit in this 64-bit hash represents one pixel comparison
    for (uint i = 0; i < 64; i++) {
        if (gray_pixels[i] > local_mean) {
            hash |= 1UL << i;
        }
    }
    
    // Save the result to the appropriate position in the output buffer
    result[thread_index] = hash;
}
"#;

/// Metal GPU context for perceptual hashing
pub struct MetalContext {
    device: metal::Device,
    command_queue: metal::CommandQueue,
    pipeline: metal::ComputePipelineState,
}

// Global Metal context, lazily initialized
static METAL_INIT: Once = Once::new();
static mut METAL_AVAILABLE: bool = false;

impl MetalContext {
    /// Create a new Metal GPU context
    pub fn new() -> Option<Self> {
        // Check if Metal is available on this system
        METAL_INIT.call_once(|| {
            autoreleasepool(|| {
                let devices = Device::all();
                unsafe { METAL_AVAILABLE = !devices.is_empty() };
            });
        });

        if !unsafe { METAL_AVAILABLE } {
            return None;
        }

        // Use autoreleasepool for proper Objective-C memory management
        autoreleasepool(|| {
            // Get default device
            let device = Device::system_default().unwrap();

            // Create command queue
            let command_queue = device.new_command_queue();

            // Create Metal library and compute function
            let library = device
                .new_library_with_source(METAL_SHADER_SRC, &metal::CompileOptions::new())
                .ok()?;
            let function = library.get_function("calculate_phash", None).ok()?;

            // Create pipeline state
            let pipeline = device
                .new_compute_pipeline_state_with_function(&function)
                .ok()?;

            Some(Self {
                device,
                command_queue,
                pipeline,
            })
        })
    }

    /// Calculate enhanced perceptual hash for an image using GPU
    pub fn calculate_phash(&self, img: &DynamicImage) -> PHash {
        // Small image optimization - use CPU for images under 1024x1024
        // This is a threshold where GPU overhead outweighs benefits
        let (width, height) = img.dimensions();
        if width < 1024 && height < 1024 {
            return crate::processing::perceptual::calculate_phash(img);
        }

        autoreleasepool(|| {
            // Create texture from image
            let descriptor = metal::TextureDescriptor::new();
            descriptor.set_width(width as u64);
            descriptor.set_height(height as u64);
            descriptor.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm);
            descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
            descriptor.set_usage(metal::MTLTextureUsage::ShaderRead);

            // Create texture
            let texture = self.device.new_texture(&descriptor);

            // Copy image data to texture
            let region = MTLSize {
                width: width as u64,
                height: height as u64,
                depth: 1,
            };

            // Extract RGBA pixels from image more efficiently
            let pixel_data = {
                let rgba = img.to_rgba8();
                rgba.into_raw()
            };

            // Upload pixel data to texture
            texture.replace_region(
                metal::MTLRegion {
                    origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
                    size: region,
                },
                0,
                pixel_data.as_ptr() as *const _,
                (width * 4) as u64, // bytes per row
            );

            // Create buffer for the result array (16 x u64 = 1024 bits)
            let result_buffer = self.device.new_buffer(
                128, // 16 * 8 bytes for u64 array
                MTLResourceOptions::StorageModeShared,
            );

            // Create command buffer and encoder
            let command_buffer = self.command_queue.new_command_buffer();
            let compute_encoder = command_buffer.new_compute_command_encoder();

            // Configure pipeline
            compute_encoder.set_compute_pipeline_state(&self.pipeline);

            // Set resource arguments
            compute_encoder.set_texture(0, Some(&texture));
            compute_encoder.set_buffer(0, Some(&result_buffer), 0);

            // Metal pipeline setup for our 16-thread kernel
            let grid_size = MTLSize {
                width: 4,
                height: 4,
                depth: 1,
            };

            // Each thread group handles 4 threads (4x4 = 16 threads total)
            let thread_group_size = MTLSize {
                width: 4,
                height: 1,
                depth: 1,
            };

            // Dispatch threads
            compute_encoder.dispatch_thread_groups(grid_size, thread_group_size);

            // End encoding
            compute_encoder.end_encoding();

            // Commit and wait for completion
            command_buffer.commit();
            command_buffer.wait_until_completed();

            // Read back result array
            let mut hash_array = [0u64; 16];
            unsafe {
                let ptr = result_buffer.contents() as *const u64;
                for i in 0..16 {
                    hash_array[i] = *ptr.add(i);
                }
            }

            // Return the enhanced hash
            PHash::Enhanced(hash_array)
        })
    }
}

// Singleton instance to avoid repeatedly creating the Metal context
struct MetalInstance {
    context: Option<MetalContext>,
}

impl MetalInstance {
    fn new() -> Self {
        Self {
            context: MetalContext::new(),
        }
    }

    fn get() -> &'static std::sync::Mutex<MetalInstance> {
        use once_cell::sync::Lazy;
        use std::sync::Mutex;

        static INSTANCE: Lazy<Mutex<MetalInstance>> =
            Lazy::new(|| Mutex::new(MetalInstance::new()));

        &INSTANCE
    }
}

/// Calculate an enhanced 1024-bit perceptual hash using Metal GPU acceleration
pub fn metal_phash(img: &DynamicImage) -> Option<PHash> {
    if let Ok(instance) = MetalInstance::get().lock() {
        if let Some(context) = &instance.context {
            Some(context.calculate_phash(img))
        } else {
            None
        }
    } else {
        None
    }
}

/// Calculate perceptual hash with GPU acceleration, falling back to CPU if needed
/// This function intelligently chooses between enhanced and standard hash based on GPU availability
pub fn gpu_accelerated_phash(img: &DynamicImage) -> PHash {
    // Get image dimensions
    let (width, height) = img.dimensions();

    // For small images, use standard CPU hash
    if width < 1024 && height < 1024 {
        return crate::processing::perceptual::calculate_phash(img);
    }

    // For larger images with GPU, use enhanced hash
    if let Some(hash) = metal_phash(img) {
        return hash; // Enhanced 1024-bit hash
    }

    // Fall back to CPU implementation if Metal is not available
    crate::processing::perceptual::calculate_phash(img)
}

/// Calculate a perceptual hash from an image file with GPU acceleration if available
/// This function intelligently chooses between enhanced and standard hash based on GPU availability
pub fn gpu_phash_from_file<P: AsRef<Path>>(path: P) -> Result<PHash, image::ImageError> {
    // Standard image opening logic
    let img = image::open(path.as_ref())?;

    // Get image dimensions
    let (width, height) = img.dimensions();

    // For small images, use standard CPU hash
    if width < 1024 && height < 1024 {
        return Ok(crate::processing::perceptual::calculate_phash(&img));
    }

    // For larger images with GPU, use enhanced hash (1024-bit)
    if let Some(hash) = metal_phash(&img) {
        Ok(hash)
    } else {
        // Fall back to CPU implementation
        Ok(crate::processing::perceptual::calculate_phash(&img))
    }
}
