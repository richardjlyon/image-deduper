use log::info;
use sysinfo::System;
use std::time::Instant;

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

    /// Check current memory usage and return (current_usage, diff_from_start)
    pub fn check_memory(&mut self) -> (u64, u64) {
        self.system.refresh_memory();
        let current_mem = self.system.used_memory();
        let diff = if current_mem > self.start_mem {
            current_mem - self.start_mem
        } else {
            0
        };
        
        (current_mem, diff)
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

/// Log memory before and after a batch operation
pub fn log_batch_memory(
    batch_idx: usize, 
    system: &mut System, 
    before_mem: u64
) -> (u64, i64) {
    system.refresh_memory();
    let after_mem = system.used_memory();
    let mem_change = (after_mem as i64 - before_mem as i64) / 1024 / 1024;
    
    info!(
        "Memory after batch {}: {}MB ({}MB change)",
        batch_idx + 1,
        after_mem / 1024 / 1024,
        if mem_change >= 0 { 
            format!("+{}", mem_change) 
        } else { 
            format!("{}", mem_change) 
        }
    );
    
    (after_mem, mem_change)
}