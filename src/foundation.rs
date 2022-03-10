use std::{cmp, env, panic};
use sysinfo::{System, SystemExt};

use crate::opt::OPT;

lazy_static! {
    static ref SYSTEM: System = {
        let mut sys = System::new_all();
        sys.refresh_all();
        sys
    };
}

#[derive(Debug)]
pub struct Foundation {
    worker_count: u64,
    slabs_per_worker: u64,
    spare_count: u64,
    spare_slabs: Vec<u64>,
}

impl Foundation {
    pub fn new() -> Foundation {
        let system_memory = system_memory();
        get_foundation_for(system_memory)
    }

    pub fn get_worker_count(&self) -> u64 {
        self.worker_count
    }

    pub fn get_slabs_per_worker(&self) -> u64 {
        self.slabs_per_worker
    }

    pub fn get_spare_count(&self) -> u64 {
        self.spare_count
    }

    /**
     * return a spare slab from the pool of spare slabs
     * TODO Stub: Write this and tests for this
     */
    pub fn get_spare_slab(&self) -> Option<bool> {
        None
    }
}

/*
* Allocate slabs as [u8, 1MB]
* use bincode to serialize entries from structs into slabs and deserialize into structs

*/
fn get_foundation_for(system_memory: u64) -> Foundation {
    if system_memory < 2097152 {
        error!("Minimum memory is 2GB");
        std::process::exit(1);
    }

    // The number of tasks is determined from (system_memory{<MB>} ÷ 60) rounded down to next power of 2
    let raw_workers = (system_memory / 1024) / 60;
    let worker_count = round_down_to_power_of_2(raw_workers);

    let working_memory = 1024 * 1024; // Allow 1GB for execution and working memory
    let tx_handle_count = cmp::min(8 * worker_count / 1024, 1024); // 8 bytes per handle, with minimum of 1MB
                                                                   // TODO Valildate average message size
    let message_size = worker_count; // Average message size of 1k
    let tokio_task_cache = 64 * worker_count / 1024;
    let reserved_memory: u64 =
        (working_memory + tx_handle_count + message_size + tokio_task_cache) as u64;
    let memory_for_slabs = system_memory - reserved_memory;
    let slabs = memory_for_slabs / (1024); // Each slab is 1MB
    let slabs_per_worker = round_down_to_power_of_2(slabs / worker_count as u64);
    let spare_count = (slabs - (worker_count as u64 * slabs_per_worker as u64)) * 2; // spare slabs are 500KB
    let spare_slabs = Vec::new();

    Foundation {
        worker_count,
        slabs_per_worker,
        spare_count,
        spare_slabs,
    }
}

fn round_down_to_power_of_2(value: u64) -> u64 {
    // Round-down to next power of two
    let mut power: u64 = 1;
    while power < value {
        power *= 2;
    }

    (power / 2)
}

fn system_memory() -> u64 {
    match OPT.get_memory() {
        Some(memory) => *memory,
        None => SYSTEM.total_memory(),
    }
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foundation() {
        let foundation = get_foundation_for(8589934);
        assert_eq!(foundation.get_worker_count(), 128);
        assert_eq!(foundation.get_slabs_per_worker(), 32);
        assert_eq!(foundation.get_spare_count(), 6536);
        assert_eq!(foundation.spare_slabs.len(), 0);
    }
}
