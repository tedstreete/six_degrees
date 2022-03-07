use std::{cmp, env};
use sysinfo::{System, SystemExt};

lazy_static! {
    static ref SYSTEM: System = {
        let mut sys = System::new_all();
        sys.refresh_all();
        sys
    };
    static ref TASKS: usize = get_worker_count(SYSTEM.total_memory());
}

#[derive(Debug)]
pub struct Slabs {
    pub slabs_per_worker: usize,
    pub spare_count: usize,
    pub spare_slabs: Vec<u64>,
}

pub fn get_worker_count(sytem_memory: u64) -> usize {
    // The number of tasks is determined from (system_memory{<MB>} ÷ 60) rounded down to next power of 2
    let raw_workers = (sytem_memory / 1024) / 60;
    round_down_to_power_of_2(raw_workers)
}

/*
* Allocate slabs as [u8, 1MB]
* use bincode to serialize entries from structs into slabs and deserialize into structs

*/
pub fn allocate_slabs(system_memory: u64) -> Slabs {
    let workers = get_worker_count(system_memory) as u64;
    let working_memory = 1024 * 1024; // Allow 1GB for execution and working memory
    let tx_handle_count = cmp::min(8 * workers / 1024, 1024); // 8 bytes per handle, with minimum of 1MB
                                                              // TODO Valildate average message size
    let message_size = workers; // Average message size of 1k
    let tokio_task_cache = 64 * workers / 1024;
    let reserved_memory: u64 =
        (working_memory + tx_handle_count + message_size + tokio_task_cache) as u64;
    let memory_for_slabs = system_memory - reserved_memory;
    let slabs = memory_for_slabs / (1024); // Each slab is 1MB
    let slabs_per_worker = round_down_to_power_of_2(slabs / workers);
    let spare_count = ((slabs - (workers * slabs_per_worker as u64)) * 2) as usize; // spare slabs are 500KB
    let spare_slabs = Vec::new();

    Slabs {
        slabs_per_worker,
        spare_count,
        spare_slabs,
    }
}

/**
 * return a spare slab from the pool of spare slabs
*/
pub fn get_spare_slab() -> Option<bool> {
    None
}

fn round_down_to_power_of_2(value: u64) -> usize {
    // Round-down to next power of two
    let mut power: u64 = 1;
    while power < value {
        power *= 2;
    }

    (power / 2).try_into().unwrap()
}

pub fn system_memory() -> u64 {
    SYSTEM.total_memory()
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
    fn test_slab_count() {
        let slabs = allocate_slabs(8589934);
        assert_eq!(slabs.slabs_per_worker, 32);
        assert_eq!(slabs.spare_count, 6536);
        assert_eq!(slabs.spare_slabs.len(), 0);
    }

    #[test]
    fn test_worker_count() {
        let worker_count = get_worker_count(8589934);
        assert_eq!(worker_count, 128);
    }
}
