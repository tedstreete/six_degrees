pub(crate) type Digest = [u8; 16];

struct Entry {
    digest: Digest,
    outbound_count: u32,
    inbound_count: u32,
    outbound: Vec<Digest>,
    inbound: Vec<Digest>,
    title: String,
}

impl Entry {
    pub fn from(source: &[u8]) {}

    pub fn to(&self) -> Vec<u8> {
        let to = Vec::new();
        to
    }
}

/* *****************************************************************************************************************
 *
 *
 * The digest is split into three fields
 *
 * xxxxssssswwwwwwww
 *
 * where:
 *    x has no special meaning
 *    s is the slab_id: The number of slabs is guaranteed to be a power of 2
 *    w is the worker_id: The number of workers is guaranteed to be a power of 2
 *
 * To determine the target worker:-
 *     do a boolean AND between the worker_id and the (number-of-tasks - 1)
 *     convert that into a u32
 *     the resulting value is the index into the Vector of TxCommands to which a request should be sent
 *
 * To determine the target slab
 *    do a boolean AND between (number_of_tasks) * (number_ of_slabs - 1)
 *    divide the result by the number of tasks
 *    convert that into a u32
 *    The resulting value is the index into the vector of slabs to be inspected
 *
 *******************************************************************************************************************/

pub fn get_digest(title: &str) -> Digest {
    md5::compute(title).into()
}

/*
 * To determine the target worker:-
 *     convert digest from [u8, 16] to u64 (discard the top 128 bits)
 *     do a boolean AND between the worker_id and the (number-of-tasks - 1)
 *     convert that into a u32
 *     the resulting value is the index into the Vector of TxCommands to which a request should be sent
 */
pub fn extract_worker_id_from(digest: Digest) -> u32 {
    0
}
