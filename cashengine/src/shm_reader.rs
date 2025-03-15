use memmap2::{MmapMut, MmapOptions};
use std::fs::File;
use std::ptr::write_bytes;
use std::time::{Duration, SystemTime};
use std::vec;
use tracing::{enabled, Level};

#[derive(Copy, Clone)]
pub struct ShareablePtr(pub(crate) *mut u8);

const ZERO_DURATION: Duration = Duration::new(0, 0);

// SAFETY: We never alias data when writing from multiple threads.
// Writer threads finish before unmapping.
unsafe impl Send for ShareablePtr {
    // The `ShareablePtr` is not aliased by any other thread.
    // This ensures that no data race occurs when accessing the `start_ptr` in multiple threads.
}

pub struct SharedMemoryReader<'a> {
    mmap_file: &'a File,
    mmap: MmapMut,
    chunk_size: usize,
    chunk_count: usize,
    file_size: usize,
    shareable_ptr: ShareablePtr,
    read_buffer: Vec<u8>,
    current_chunk_id: usize,
}

impl<'a> SharedMemoryReader<'a> {
    pub fn create(
        mmap_file: &'a File,
        chunk_size: usize,
        chunk_count: usize,
    ) -> SharedMemoryReader<'a> {
        let file_size = chunk_size * chunk_count;
        let mut mmap = SharedMemoryReader::map_file_to_memory(&mmap_file, file_size);
        let start_ptr =
            SharedMemoryReader::initialize_start_ptr_to_mapped_memory(&mut mmap, file_size);
        let shareable_ptr = ShareablePtr(start_ptr);
        let shm_reader = SharedMemoryReader {
            mmap_file,
            mmap,
            chunk_size,
            chunk_count,
            file_size,
            shareable_ptr,
            read_buffer: vec![0u8; chunk_size],
            current_chunk_id: 0,
        };
        shm_reader
    }

    fn map_file_to_memory(file: &File, file_size: usize) -> MmapMut {
        tracing::info!("Mapping SHM file to memory for reading");
        unsafe {
            match MmapOptions::new().offset(0).len(file_size).map_mut(file) {
                Ok(mmap) => mmap,
                Err(e) => {
                    panic!("Failed to map SHM file to memory for reading: {}", e);
                }
            }
        }
    }

    fn initialize_start_ptr_to_mapped_memory(mmap: &mut MmapMut, file_size: usize) -> *mut u8 {
        tracing::info!("Initializing SHM file with zeros for reading");
        let start_ptr = mmap.as_mut_ptr();
        unsafe {
            write_bytes(start_ptr.offset(0), 0u8, file_size);
        }
        tracing::info!("Got for reading the start_ptr: {:p}", start_ptr);
        start_ptr
    }

    pub fn read_next_message(&mut self) -> &[u8] {
        let start_ptr: *mut u8 = self.shareable_ptr.0;

        let target_offset = self.current_chunk_id * self.chunk_size;

        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

        let read_start = self.start_bench();

        unsafe {
            // SAFETY: We never overlap on writes.
            // Pointer is living because we use scoped threads.
            std::ptr::copy_nonoverlapping(
                start_ptr.wrapping_offset(target_offset as isize),
                self.read_buffer.as_ptr().cast_mut(),
                self.chunk_size,
            );
        }

        if enabled!(Level::TRACE) {
            let read_duration = self.end_bench(read_start);
            tracing::trace!(
                "SharedMemoryReader read chunk_id {} at offset {} in {} μs",
                self.current_chunk_id,
                target_offset,
                read_duration.as_micros()
            );
        }

        self.next_chunk();
        &self.read_buffer[..self.chunk_size]
    }

    fn next_chunk(&mut self) {
        self.current_chunk_id += 1usize;
        if self.current_chunk_id >= self.chunk_count {
            self.current_chunk_id = 0usize;
        }
    }

    fn end_bench(&self, read_start: SystemTime) -> Duration {
        if enabled!(Level::TRACE) {
            let read_end = SystemTime::now();
            let read_duration = read_end.duration_since(read_start);
            read_duration.unwrap_or_else(|e| {
                tracing::error!("SharedMemoryReader failed getting duration for read: {}", e);
                ZERO_DURATION
            })
        } else {
            ZERO_DURATION
        }
    }

    fn start_bench(&self) -> SystemTime {
        SystemTime::now()
    }
}
