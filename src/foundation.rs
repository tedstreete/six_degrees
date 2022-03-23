//! Determine foundational attributes based on available system memory

use std::cmp::{max, min};
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
    worker_count: u32,
    //    bits_for_workers: u16,
    bitwise_worker_match: u16,
    slabs_per_worker: u32,
    //  bits_for_slabs: u16,
    bitwise_slab_match: u16,
    spare_count: u64,
    spare_slabs: Vec<u64>,
}

impl Foundation {
    pub fn new() -> Foundation {
        get_foundation_for(system_memory(), system_cores(), raw_workers())
    }

    // pub fn get_bits_for_workers(&self) -> u16 {
    //     self.bits_for_workers
    // }

    pub fn get_bitwise_worker_match(&self) -> u16 {
        self.bitwise_worker_match
    }

    pub fn get_worker_count(&self) -> u32 {
        self.worker_count
    }

    // pub fn get_bits_for_slabs(&self) -> u16 {
    //     self.bits_for_slabs
    // }

    pub fn get_bitwise_slab_match(&self) -> u16 {
        self.bitwise_slab_match
    }

    pub fn get_slabs_per_worker(&self) -> u32 {
        self.slabs_per_worker
    }

    /// Returns the number of unallocated spare slabs
    pub fn get_spare_count(&self) -> u64 {
        self.spare_count
    }

    /// Allocate a spare slab from the pool of spare slabs
    ///
    /// TODO Stub: Write this and tests for this

    pub fn get_spare_slab(&self) -> Option<bool> {
        // Don't forget to reduce the spare_slabs count when allocating
        None
    }

    pub fn extract_worker_id_from(&self, digest: crate::entry::Digest) -> u16 {
        let mut id: u16 = digest[1].into();
        id = id << 8;
        id += digest[0] as u16;
        id & self.get_bitwise_worker_match()
    }

    pub fn extract_slab_id_from(&self, digest: crate::entry::Digest) -> u16 {
        let mut id: u16 = digest[3].into();
        id = id << 8;
        id += digest[2] as u16;
        id & self.get_bitwise_slab_match()
    }
}

/*
* Allocate slabs as [u8, 1MB]
* use bincode to serialize entries from structs into slabs and deserialize into structs

*/
fn get_foundation_for(system_memory: u64, cores: usize, raw_workers: u32) -> Foundation {
    // All memory calculations in KB
    if system_memory < 2097152 {
        error!("Minimum memory is 2GB");
        std::process::exit(1);
    }

    let worker_count = round_down_to_power_of_2(raw_workers);

    // 8 bytes per handle, with at least 1MB
    let tx_handle_count: u32 = max(8 * worker_count as u32 / 1024, 1024);

    let working_memory: u32 = 1024 * 1024; // Allow 1GB for execution and working memory

    // TODO Valildate average message size
    let message_size: u32 = worker_count as u32; // Average message size of 1k
    let tokio_task_cache: u32 = 64 * worker_count as u32 / 1024;
    let reserved_memory: u64 =
        (working_memory + tx_handle_count + message_size + tokio_task_cache) as u64;
    let memory_for_slabs = system_memory - reserved_memory;

    // slab_id must fit into 16 bits, so max number of slabs is u16::MAX
    let slabs: u32 = min(
        (memory_for_slabs / (1024)).try_into().unwrap(),
        (u16::MAX as u64).try_into().unwrap(),
    ); // Each slab is 1MB
    let slabs_per_worker = round_down_to_power_of_2(slabs / worker_count);
    let spare_count: u64 = ((slabs - (worker_count * slabs_per_worker)) * 2).into(); // spare slabs are 500KB
    let spare_slabs = Vec::new();

    //   let bits_for_workers = required_bits_for(worker_count - 1);
    //   let bits_for_slabs = required_bits_for(slabs_per_worker - 1);

    Foundation {
        worker_count,
        slabs_per_worker,
        spare_count,
        spare_slabs,
        bitwise_worker_match: (worker_count - 1).try_into().unwrap(),
        bitwise_slab_match: (slabs_per_worker - 1).try_into().unwrap(),
    }
}

/*
This function is probably no longer needed. Will keep it around for a few cycles incase it is needed

fn required_bits_for(val: u32) -> u16 {
    let mut count = 0;
    let mut val = val;

    while val > 0 {
        count += 1;
        val >>= 1;
    }
    count
}
*/

fn round_down_to_power_of_2(value: u32) -> u32 {
    // Round-down to next power of two
    let mut power: u32 = 1;
    while power <= value as u32 {
        power *= 2;
    }

    power / 2
}

fn system_memory() -> u64 {
    match OPT.get_memory() {
        Some(memory) => *memory,
        None => SYSTEM.total_memory(),
    }
}

fn system_cores() -> usize {
    match OPT.get_cores() {
        Some(cores) => *cores as usize,
        None => SYSTEM.physical_core_count().unwrap(),
    }
}

fn raw_workers() -> u32 {
    // Use cores * 2 to account for hyperthreading that may be enabled on some processor architectures
    // Over-allocating tasks on a non-hyperthreaded processor will not have a meaningful impact
    // worker count cannot exceed 65K workers (16 bits)
    match OPT.get_worker_count() {
        Some(raw_workers) => raw_workers,
        None => min(system_cores() * 2, u16::MAX.into()) as u32,
    }
}

/* *****************************************************************************************************************
 *
 * Tests
 *
 * *****************************************************************************************************************/

// Module is public, as get_test_foundation is called from test functions in worker.rs
#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_get_worker_count() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_worker_count(), 16);
    }

    #[test]
    fn test_get_bitwise_worker_match() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_bitwise_worker_match(), 15);
    }

    #[test]
    fn test_get_slabs_per_worker() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_slabs_per_worker(), 256);
    }

    #[test]
    fn test_get_bitwise_slab_match() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_bitwise_slab_match(), 255);
    }

    #[test]
    fn test_get_spare_count() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.get_spare_count(), 6534);
    }

    #[test]
    fn test_spare_slabs() {
        let foundation = get_test_foundation();
        assert_eq!(foundation.spare_slabs.len(), 0);
    }

    #[test]
    fn test_extract_worker_id_from() {
        let foundation = get_test_foundation();
        let digest = crate::entry::Entry::get_digest("Rail transport");
        assert_eq!(foundation.extract_worker_id_from(digest), 11);
    }

    #[test]
    fn test_extract_slab_id_from() {
        let foundation = get_test_foundation();
        let digest = crate::entry::Entry::get_digest("Rail transport");
        assert_eq!(foundation.extract_slab_id_from(digest), 196);
    }

    #[test]
    fn test_round_down() {
        assert_eq!(round_down_to_power_of_2(31), 16);
        assert_eq!(round_down_to_power_of_2(17), 16);
        assert_eq!(round_down_to_power_of_2(16), 16);
    }

    /* *****************************************************************************************************************
     *
     * Helper functions - Used only by test routines, but need to be public so that they are accessible from other
     * modules
     * *****************************************************************************************************************/

    /// Create a default Foundation struct with a small memory footprint that will not exhaust available memory,
    /// leaving sufficient memory for developer tools to run alongside the tests

    pub fn get_test_foundation() -> Foundation {
        get_foundation_for(8589934, 8, 16)
    }
}
