use log::info;
use std::time::Instant;
use sysinfo::System;

/// Simple memory tracking utility
pub struct MemoryTracker {
    system: System,
    start_mem: u64,
    last_check: Instant,
}

impl MemoryTracker {
    /// Create a new memory tracker and initialize it
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let start_mem = system.used_memory();

        Self {
            system,
            start_mem,
            last_check: Instant::now(),
        }
    }

    /// Check and log memory usage if sufficient time has passed
    pub fn log_memory(&mut self, label: &str) -> (u64, u64) {
        self.system.refresh_memory();
        let current_mem = self.system.used_memory();
        let diff = if current_mem > self.start_mem {
            current_mem - self.start_mem
        } else {
            0
        };

        // Only log if enough time has passed since last check (1 second)
        if self.last_check.elapsed().as_secs() >= 1 {
            info!(
                "Memory at {}: current={}MB, diff=+{}MB",
                label,
                current_mem / 1024 / 1024,
                diff / 1024 / 1024
            );
            self.last_check = Instant::now();
        }

        (current_mem, diff)
    }
}
