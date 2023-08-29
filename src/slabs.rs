/* ********************************************************************************************************************
 *
 * Single slab strategy
 *
 * ********************************************************************************************************************
 *
 * Tiered memory strategy
 *
 * There are two types of slabs
 *
 * 1. Entries
 *    Contain an array of fixed-length entries
 *    {
 *       digest: The digest of the Entry (128 bytes)
 *       links: pointer to the appropriate Links struct (8 bytes)
 *    }
 *
 * 2. Links
 *    A list of variable length objects, containing
 *    Title: The page titile: Used to determine an exact match in the event of duplicate entries
 *    inbound links:
 *    outbound links:
 *
 * On startup, each worker will have
 *
 *   32 entry slabs of 557,056 ( (128+8) * 4096 entries ) bytes in length
 *   32 Links slabs of 1024 * 1024 bytes
 *
 * If space becomes exhausted on either slab, the worker can request an extension slab
 *
 * This model becomes problematic if the objects in the Links slab nee to be moved becasuse the number of links has changed
 *
 *********************************************************************************************************************/
