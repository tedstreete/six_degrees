pub(crate) type Digest = [u8; 16];

#[derive(Debug)]
pub struct Entry {
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

    pub fn get_digest(title: &str) -> Digest {
        md5::compute(title).into()
    }
}

/* *****************************************************************************************************************
 *
 * Digest[0;2] hold the worker_id. If there are less than 65K workers, then the additional bits are ignored when
 * determining the id. However the bits are still significant when matching the overall digest.
 *
 * Digest[2;2] hold the slab_id. If there are less than 65K workers, then the additional bits are ignored when
 * determining the id. However the bits are still significant when matching the overall digest.
 *
 * The endiness of the processor is not significant. Providing the converstion between It will not
 * matter whether the conversion uses big-endian or little-endian, providing all the conversions are consistent.
 *
 * To determine the target worker:-
 *     Create u16 from Digest[0;2]
 *     do a boolean AND between the resulting u16 and the (worker_count - 1)
 *     the resulting value is the index into the Vector of TxCommands to which a request should be sent
 *
 * To determine the target slab
 *    Create u16 from Digest[2;2]
 *    do a boolean AND between the resulting u16 and (number_ of_slabs - 1)
 *    The resulting value is the index into the vector of slabs to be inspected
 *
 *******************************************************************************************************************/

/*
 * To determine the target worker:-
 *     convert digest from [u8, 2] to u16 (discard the top 112 bits)
 *     do a boolean AND between the worker_id and the (foundation.worker_count - 1)
 *     the resulting value is the index into the Vector of TxCommands to which a request should be sent
 */
