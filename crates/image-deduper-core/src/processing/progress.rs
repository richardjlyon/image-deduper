use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::info;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::System;

/// Progress tracker for image processing operations
pub struct ProgressTracker {
    /// Total number of images to process
    total: usize,
    /// Main progress bar showing overall progress
    main_progress: ProgressBar,
    /// Current batch progress bar
    batch_progress: ProgressBar,
    /// Multi-progress for displaying both progress bars
    _multi_progress: Arc<MultiProgress>,
    /// Start time of the operation
    start_time: Instant,
    /// Memory usage tracking
    system: Mutex<System>,
    /// Starting memory usage in MB
    start_memory_mb: u64,
    /// Peak memory usage in MB
    peak_memory_mb: Mutex<u64>,
    /// Initial position of the progress bar
    initial_position: u64,
    /// Batch start time to calculate batch-specific rates
    batch_start_time: Mutex<Instant>,
    /// Images processed in current batch
    batch_processed: Mutex<usize>,
    /// Recent processing rate (images/second)
    recent_rate: Mutex<f64>,
}

impl ProgressTracker {
    /// Create a new progress tracker for the given number of images
    ///
    /// * `total_images` - The total number of images (already processed + to process)
    /// * `initial_position` - Number of images already processed
    /// * `initial_successful` - Number of successful images processed
    /// * `initial_errors` - Number of failed image processings
    pub fn new(
        total_images: usize,
        initial_position: usize,
        initial_successful: usize,
        _initial_errors: usize,
    ) -> Self {
        let multi_progress = Arc::new(MultiProgress::new());

        // Create the main progress bar for overall progress
        let main_progress = multi_progress.add(ProgressBar::new(total_images as u64));
        main_progress.set_style(
            ProgressStyle::default_bar()
                .template("{wide_bar} {pos}/{len} ({percent}%) | {msg}")
                .unwrap()
                .progress_chars("█▓▒░ "),
        );
        // Set initial position and message
        main_progress.set_position(initial_position as u64);

        // Calculate initial stats message
        if initial_position > 0 {
            let message = format!(
                "Processing... | {} already in DB | 0.0 img/s",
                initial_successful
            );
            main_progress.set_message(message);
        } else {
            main_progress.set_message("Processing...");
        }

        // Create an invisible batch progress bar (we'll use it for tracking but not display)
        let batch_progress = ProgressBar::new(100);
        batch_progress.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        // Initialize memory tracking
        let mut system = System::new_all();
        system.refresh_all();
        let start_memory_mb = system.used_memory() / 1024 / 1024;

        let now = Instant::now();

        Self {
            total: total_images,
            main_progress,
            batch_progress,
            _multi_progress: multi_progress,
            start_time: now.clone(),
            system: Mutex::new(system),
            start_memory_mb,
            peak_memory_mb: Mutex::new(start_memory_mb),
            initial_position: initial_position as u64,
            batch_start_time: Mutex::new(now),
            batch_processed: Mutex::new(0),
            recent_rate: Mutex::new(0.0),
        }
    }

    /// Start a new batch of images
    pub fn start_batch(&self, batch_size: usize, batch_number: usize, total_batches: usize) {
        self.batch_progress.reset();
        self.batch_progress.set_length(batch_size as u64);
        self.batch_progress
            .set_message(format!("Batch {}/{}", batch_number, total_batches));

        // Reset batch processing counters
        *self.batch_start_time.lock().unwrap() = Instant::now();
        *self.batch_processed.lock().unwrap() = 0;

        // Update memory information
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        let current_mem = system.used_memory() / 1024 / 1024;

        // Update peak memory if needed
        let mut peak = self.peak_memory_mb.lock().unwrap();
        if current_mem > *peak {
            *peak = current_mem;
        }

        // We don't log memory information to the console anymore
        info!(
            "Memory: {}MB (peak: {}MB, change: {:+}MB)",
            current_mem,
            *peak,
            current_mem as i64 - self.start_memory_mb as i64
        );
    }

    /// Update progress for the batch
    pub fn update_batch(&self, processed: usize, status: &str) {
        self.batch_progress.set_position(processed as u64);
        self.batch_progress.set_message(status.to_string());

        // Update batch processed count
        *self.batch_processed.lock().unwrap() = processed;
    }

    /// Complete a batch processing
    pub fn complete_batch(&self, successful: usize, errors: usize) {
        // We don't display batch progress anymore
        self.batch_progress.finish();

        // Calculate the processing rate for this batch
        let batch_elapsed = self
            .batch_start_time
            .lock()
            .unwrap()
            .elapsed()
            .as_secs_f64();

        // Use the actual count from successful/errors parameters
        let batch_processed = successful + errors;

        if batch_elapsed > 0.0 && batch_processed > 0 {
            let rate = batch_processed as f64 / batch_elapsed;
            *self.recent_rate.lock().unwrap() = rate;

            info!("Batch processing rate: {:.1} img/s", rate);
        }

        // Update memory information
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        let current_mem = system.used_memory() / 1024 / 1024;

        // Update peak memory if needed
        let mut peak = self.peak_memory_mb.lock().unwrap();
        if current_mem > *peak {
            *peak = current_mem;
        }

        // Only log to file, not console
        info!(
            "Memory after batch: {}MB (peak: {}MB, change: {:+}MB)",
            current_mem,
            *peak,
            current_mem as i64 - self.start_memory_mb as i64
        );
    }

    /// Update the main progress with latest count values
    pub fn increment(&self, successful: usize, errors: usize) {
        // Get the recent processing rate
        let rate = *self.recent_rate.lock().unwrap();

        // Use the most recent batch rate if available, otherwise use a reasonable default
        let speed = if rate > 0.0 {
            rate
        } else {
            // Default to a reasonable rate if no batches have been processed yet
            25.0
        };

        // Current total progress
        let position = successful as u64;
        self.main_progress.set_position(position);

        // Calculate remaining images for ETA
        let remaining = self.total as u64 - position;
        let eta_secs = if speed > 0.0 {
            (remaining as f64 / speed) as u64
        } else {
            0
        };

        // Format ETA
        let eta = if eta_secs < 60 {
            format!("{}s", eta_secs)
        } else if eta_secs < 3600 {
            format!("{}m {}s", eta_secs / 60, eta_secs % 60)
        } else {
            format!("{}h {}m", eta_secs / 3600, (eta_secs % 3600) / 60)
        };

        self.main_progress.set_message(format!(
            "{:.1} img/s | {} ok | {} errors | ETA: {}",
            speed, successful, errors, eta
        ));
    }

    /// Complete the progress tracking
    pub fn finish(&self, successful: usize, errors: usize) {
        // Calculate final stats
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let newly_processed = self.main_progress.position() - self.initial_position;
        let throughput = if elapsed > 0.0 && newly_processed > 0 {
            newly_processed as f64 / elapsed
        } else {
            0.0
        };

        // Update memory information
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        let _final_mem = system.used_memory() / 1024 / 1024;
        let _peak = *self.peak_memory_mb.lock().unwrap();

        // Complete the progress bar (batch progress is hidden)
        self.main_progress.finish_with_message(format!(
            "Completed {} images | {} ok | {} errors | {:.1}s elapsed | {:.1} img/s",
            successful + errors,
            successful,
            errors,
            elapsed,
            throughput
        ));
    }
}
