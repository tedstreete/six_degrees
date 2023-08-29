# Development notes

## Dependencies
1. zeromq -> `sudo apt install libzmq3-dev`
2. messagepack: http://msgpack.org/index.html
 1. rust: https://github.com/3Hren/msgpack-rust
 2. c:    https://github.com/msgpack/msgpack-c
2. sclog4c -> https://github.com/christianhujer/sclog4c


## Architecture

### IO Thread

uses pub-sub to clients
uses pub-sub to Threads

pseudo code
```
while (true)
	setup zmq_poll_items array, one for extenal comms, and one for each of the worker threads
	zmq_poll ()
	if (external IO) {
		send request to next thread, in round-robin fashion
		request includes ID of external client
	}
	else { // must be worker thread
		send message
	}
```

each thread owns 1/(number of threads) slabs. Use messages to request records from each thread (or update slabs). No need for locking or other memory semantics. Use a lookup table to determine the transition point between each thread/slab based on the hash associated with each wikipedia page. Thread count is 2 * (number of cores). IO dispatcher fires off a thread using pseudo code above and that thread sends messages to each slab thread.

This model allows future implementations to move IO thread and worker_threads to a separate server if necessary,

___OR___ have a server process just to implement the slab_threads, putting the IO_thread and worker_threads into a separate process which can be easily moved to a different physical server if necessary. We now have a three tier model -
1. Web: (__Rust__) Communicates with the client browser; includes the IO_thread
2. App: (__Rust__) IO_thread and Worker_threads
3. Data: (__C__) slab_threads. One thread for every core, rounded up to the next power of 2.

The threading model now looks like:-

1. IO thread. Communicates with clients using pub-sub. Accepts requests, puts request into request block (two statically allocated for each worker thread); sends a reference to the request block to the associated thread. gets response from worker thread and forwards response to client. marks the request_block as empty ready for next request. request_block is owned by the IO_thread until it's sent to the worker_thread. It's owned by the worker_thread until the response is sent back to the IO_thread. responsible for releasing the memory allocated by the worker_thread (pointer to the memory is in the request_block).
2. Worker_thread. 1 for every core in the server. accepts a message with a reference to a request_block. marshals a response, sends requests as necessary to slab_threads, using filters to direct the message to the right one (filter is the LSB of each hash). allocates memory for the response. sends the response back to the IO_thread using a pub-sub model
3. slab_thread. Must be a power of 2 in number, as slab_threads use the LSB of each hash to filter which slab_thread is responsible for processing the request. Note that the hash LSB is a convenience for a selector, and has no relationship to the has functions of the pages in the slabs for which it is responsible. The PageIdentity table is similarly divided into pages (even though it's allocated as a contiguous unit) and manipulated by the slab_threads. The boundaries between responsibilities is between CollisionChain entries (i.e. we can use the same selector of the has LSB to determine which slab_thread is responsible for which entry in the PageIdentity table). This model allows slabs to be distributed across multiple servers if necessary, the only limitation being that the _total number of threads is always a power of 2_.

___Something to think about. Can we combine the web tier and the app tier?___

### loud thinking

All worker and slab threads publish and subscribe on two Publish-subscribe sessions

1. from worker to slab - filter on hex number for the sub-hash identifier for the target slab thread
2. from slab to worker - filter on the thread number (assigned when the worker thread is started. The request from the worker to the slab includes the filter identifier)

#### filters
- slab threads are identified by 4 hex characters allowing a maximum of 64K slab threads in a single system, spanning multiple physical machines if necessary. There's no limit on how the threads are placed on the physical machines (there's no need for them to be evenly spread), but the total number of slab threads *must* be an exact power of two. The sub-hash value is determined by the LSB of the md5 hash for the page (e.g. the bottom 2 bits = 4 threads, bottom 5 bits = 32 threads etc.)

#### slab thread

- receive filtered message (Filter: "nnnn", where nnnn is the hex number for the sub-hash identifier - this gives a total of 65,536 slab threads, making it easy to span multiple machines if necessary)
- switch (read or save)
 - read
  - find page in PageLinks table associated with the sub-hash
  - return page (or null if page doesn't exist) to the worker thread identified in the request
 - save
   - find space in PageLinks table
	- put data in PageLinks table
	- put pointer to data in appropriate CollisionChain in PageIdentity table

#### worker thread

- send request to slab thread with filter set by sub-hash identifier

### Auditing.

Each thread logs the time it starts to wait for a request, and the time it starts after a request. Can use that information to determine how evenly the workload is spread across threads in each group.



### PageIdentity
```
struct PageIdentity {
	uint_64 md5Lo; // the least significant 64 bits of the md5 hash for the target page
	void *pageLink; // entry for that page in the PageLinks table.
}
```

### CollisionChain

A CollisionChain is an array of struct PageIdentity. The number of PageIdentity entries in each CollisionChain is determined at run time. Use a linear search for all the entries in the CollisionChain to find a PageIdentity that matches a given hash. Collisions in the hash are resolved by comparing the target page title with the page title saved in the struct PageIdentity.


### PageHash table

The PageHash table is an array of CollisionChain entries

The size of this table is determined at runtime, and is a function of

- the number of CollisionChain elements - this is determined by the number of significant bits in the MD5 hash and is given by 2^significantBits
- the number of PageIdentity elements in each CollisionChain entry

| Significant bits | Entries in hashtable | Size of each PageIdentity | Number of entries in collision chain | Size of each element in CollisionChain array |Memory required (MB) |
| :------------- | :------------- | :------------- | :------------- | :------------- | :------------- |
| 20 | 1048576 | 16 | 8 | 128 | 128 |
| 21 | 2097152 | 16 | 8 | 128 | 256 |
| 22 | 4194304 | 16 | 8 | 128 | 512 |
| 23 | 8388608 | 16 | 8 | 128 | 1024 |
| 24 | 16777216 | 16 | 8 | 128 | 2048 |
| | | | | | |
| 20 | 1048576 | 16 | 12 | 192 | 192 |
| 21 | 2097152 | 16 | 12 | 192 | 384 |
| 22 | 4194304 | 16 | 12 | 192 | 768 |
| 23 | 8388608 | 16 | 12 | 192 | 1536 |
| 24 | 16777216 | 16 | 12 | 192 | 3072 |
| | | | | | |
| 20 | 1048576 | 16 | 16 | 256 | 256 |
| 21 | 2097152 | 16 | 16 | 256 | 512 |
| 22 | 4194304 | 16 | 16 | 256 | 1024 |
| 23 | 8388608 | 16 | 16 | 256 | 2048 |
| 24 | 16777216 | 16 | 16 | 256 | 4096 |
| | | | | | |
| 20 | 1048576 | 16 | 20 | 320 | 320 |
| 21 | 2097152 | 16 | 20 | 320 | 640 |
| 22 | 4194304 | 16 | 20 | 320 | 1280 |
| 23 | 8388608 | 16 | 20 | 320 | 2560 |
| 24 | 16777216 | 16 | 20 | 320 | 5120 |

The offset into the PageHash table for a given Wikipedia page is determined by the least significant bits of the calculated hash. A linear search of the PageIdentity elements in the CollisionChain is used to find the matching page

__Allocation strategy__ for the size of PageHash and the length of the CollisionChain.

- For a given number of potential PageIdentity elements, there is a trade-off between the size of the PageHash table and the length of the CollisionChain.
- Increasing the number of significant bits, resulting in a larger PageHash table, at the expense of the number of elements in each CollisionChain will reduce the mean time to find a given page, but unless the hash values are absolutely evenly distributed, this will result in greater wasted space, as more potential hash values will be unused.
- Conversely, increasing the CollisionChain length at the expense of the PageHash table size reduces wasted memory resources, but increases the mean time to find a given page, as more candidate PageHash elements will need to be inspected before the target is located.

### PageLinks table
```
struct PageLinks {
	char[] URL; // The url of this page (zero terminated string)
	// If necessary, padding is inserted here to align the count with a 64 bit boundary
	uint_64 count; // The number of entries in the Links array
	uint_64[] toLinks; // the LSB 64 bits hash for the Wikipedia pages referenced by this page
}
```
The PageLinks table will grow to the available memory using rawSlabs to add more space when necessary. When adding a new Pagelink element, each slab is inspected, and the PageLink element is saved in the first slab with sufficient space for that element.

There are no gaps in the PageLinks table. Entries following an entry that is deleted are moved-up to fill the gaps.

### Links are unidirectional

- No need for a fromLinks chain. When we look for paths between the two pages, we’ll follow the toLinks from both pages to identify the paths in both directions. In most cases, the two paths will be symmetrical, but occasionally they will be non-symmetrical, or possibly unreachable in one direction.

## API

API protocol uses JSON formatted strings.

 - stopServer (aa): Stops the server, saving the dataset to a file. Don’t exit if the dataset was not saved successfully, unless the forceExit parameter is true
	Parameters
		0: Method (aa = stopServer)
		1: forceExit (boolean) If true, exit even if the dataset save was not successful
		2: (optional) filename. If omitted, uses the default location for the dataset
	Returns
		0: Success
		-1: Insufficient space to save dataset

 - saveDataset (ab): Saves the dataset to a file
	Parameters
		0: Method (ab = saveDataset)
		1: (optional) filename. If omitted, uses the default location for the dataset
	Returns
		0: Success
		-1: Insufficient space to save dataset

 - updatePage (ac): Adds a page to the dataset. If the page already exists, overwrite the existing entry
	Parameters
		0: Method (ac = addPage)
		1: Title (String)
		2+: Array of linked titles (String)
	Returns
		0: success
		-1: Insufficient memory
		-2: Incorrectly formatted page

 - getLinks (ad): Provides all the pages that are within (n) links of this page
	Parameters
		0: Method (ac = 6Degrees)
		1: Title (String)
		2: Number of degrees (String)
	Returns:
		0: + graph of all pages that are within the requested number of degrees
		-1: Page doesn't exist
		-2: Too many degrees requested. Initially, we'll limit the number of degrees to a maximum of 6. Can revisit this decision later

 - pathsBetween (ad): Provides all the possible paths between the two pages
	Parameters
		0: Method (ad = pathsBetween)
		1: Source (String)
		1: Target (String)
	Returns:
		0: + graph of the shortest link(s) between Source and Target
		-1: No links were found
		-2 :Source doesn't exist
		-3: Target doesn't exist


## Random notes

/*****************************************************************************************************************
*
* abort.c
* Copyright (C) 2017 Ted <dev@streete.org>
*
* abort () can be called at any time, during initialization or operation. It will appropriately
* clean-up, release any resources that have been successfully allocated, log the error message
* to the system log (if possible) and then quit.
*
*-----------------------------------------------------------------------------------------------
*
* SixDegrees is free software: you can redistribute it and/or modify it
* under the terms of the GNU General Public License as published by the
* Free Software Foundation, either version 3 of the License, or
* (at your option) any later version.
*
* SixDegrees is distributed in the hope that it will be useful, but
* WITHOUT ANY WARRANTY; without even the implied warranty of
* MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
* See the GNU General Public License for more details.
*
* You should have received a copy of the GNU General Public License along
* with this program.  If not, see <http://www.gnu.org/licenses/>.
*
******************************************************************************************************************/


/*****************************************************************************************************************
*
* Application entry point
*
******************************************************************************************************************/



### CMake

If you have CMake 3.1.0+, this becomes even easier:

---
set(THREADS_PREFER_PTHREAD_FLAG ON)
find_package(Threads REQUIRED)
target_link_libraries(my_app Threads::Threads)
---

If you are using CMake 2.8.12+, you can simplify this to:

find_package(Threads REQUIRED)
if(THREADS_HAVE_PTHREAD_ARG)
  target_compile_options(PUBLIC my_app "-pthread")
endif()
if(CMAKE_THREAD_LIBS_INIT)
  target_link_libraries(my_app "${CMAKE_THREAD_LIBS_INIT}")
endif()

###JSON libraries
https://github.com/udp/json-parser
https://github.com/udp/json-builder
https://github.com/mnunberg/jsonsl
https://github.com/DaveGamble/cJSON
http://zserge.com/jsmn.html
