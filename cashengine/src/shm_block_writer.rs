use crate::util::MAX_USIZE_STRING_LENGTH;
use memmap2::{MmapMut, MmapOptions};
use std::fmt::Write;
use std::fs::File;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Copy, Clone)]
pub struct ShareablePtr(pub(crate) *mut u8);

// SAFETY: We never alias data when writing from multiple threads.
// Writer threads finish before unmapping.
unsafe impl Send for ShareablePtr {
    // The `ShareablePtr` is not aliased by any other thread.
    // This ensures that no data race occurs when accessing the `start_ptr` in multiple threads.
}

pub struct SharedMemoryWriter<'a> {
    sequence: usize,
    mmap_file: &'a File,
    mmap: MmapMut,
    writer_id: usize,
    chunk_size: usize,
    shareable_ptr: ShareablePtr,
    write_buffer: String,
}

impl<'a> SharedMemoryWriter<'a> {
    pub fn create(
        mmap_file: &'a File,
        writer_id: usize,
        chunk_size: usize,
        chunk_count: usize,
    ) -> SharedMemoryWriter<'a> {
        let block_size = chunk_size * chunk_count;
        let mut mmap = SharedMemoryWriter::map_file_to_memory(mmap_file, writer_id, block_size);
        let start_ptr =
            SharedMemoryWriter::initialize_start_ptr_to_mapped_memory(&mut mmap, writer_id);
        let shareable_ptr = ShareablePtr(start_ptr);
        let shm_writer = SharedMemoryWriter {
            sequence: 0,
            mmap_file,
            mmap,
            writer_id,
            chunk_size,
            shareable_ptr,
            write_buffer: String::with_capacity(chunk_size),
        };
        shm_writer
    }

    fn map_file_to_memory(file: &File, writer_id: usize, block_size: usize) -> MmapMut {
        tracing::info!("Mapping file to memory for writer_id {}", writer_id);
        unsafe {
            match MmapOptions::new()
                .offset((writer_id * block_size) as u64)
                .len(block_size)
                .map_mut(file)
            {
                Ok(mmap) => mmap,
                Err(e) => {
                    panic!("Failed to map SHM file to memory for writer_id {}: {}", writer_id, e);
                }
            }
        }
    }

    fn initialize_start_ptr_to_mapped_memory(mmap: &mut MmapMut, writer_id: usize) -> *mut u8 {
        tracing::info!("Initializing SHM file for writer_id {}", writer_id);
        let start_ptr = mmap.as_mut_ptr();
        tracing::info!("Got for writer_id {} the start_ptr: {:p}", writer_id, start_ptr);
        start_ptr
    }

    pub fn write(&mut self, chunk_index: usize, message: &[u8]) {
        let start_ptr: *mut u8 = self.shareable_ptr.0;
        self.write_buffer.clear();

        let start_timestamp_micros = self.start_bench();

        let target_offset = chunk_index * self.chunk_size;
        // TODO: Remove String allocation and this write!() here. Use copy_nonoverlapping() to directly copy the message into the write_buffer.
        let mut message = String::from_utf8(message.to_vec()).unwrap();
        message.push('\0');

        write!(
            &mut self.write_buffer,
            "{}:{}:{}:{:0width$}:{}",
            self.writer_id,
            self.sequence,
            start_timestamp_micros,
            self.writer_id + (target_offset),
            message,
            width = MAX_USIZE_STRING_LENGTH
        )
        .unwrap();
        if self.write_buffer.len() > self.chunk_size {
            panic!("SharedMemoryWriter writer_id {} write_buffer size {} is greater than chunk size: {}",
                   self.writer_id, self.write_buffer.len(), self.chunk_size);
        }
        /*tracing::trace!(
            "SharedMemoryWriter writer_id {} writing to offset {} at time {}",
            self.writer_id,
            self.shareable_ptr.0.addr() + target_offset,
            start_timestamp_micros
        );
        let write_start = SystemTime::now();*/

        unsafe {
            // SAFETY: We never overlap on writes.
            // Pointer is living because we use scoped threads.
            let target_ptr = start_ptr.add(target_offset);
            std::ptr::copy_nonoverlapping(
                self.write_buffer.as_ptr(),
                target_ptr,
                self.chunk_size,
            );
        }

        /*
        let write_duration = self.end_bench(write_start);
        tracing::trace!(
            "SharedMemoryWriter writer_id {} wrote at offset {} at time {}. Write took {} μs",
            self.writer_id,
            start_ptr.addr(),
            micro_timestamp,
            write_duration.as_micros()
        );*/

        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
        self.sequence += 1;
    }

    pub fn close(&self) {
        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    }

    fn start_bench(&self) -> u128 {
        let current_system_time = SystemTime::now();
        let mut timestamp_micros = 0;
        match current_system_time.duration_since(UNIX_EPOCH) {
            Ok(duration_since_epoch) => {
                timestamp_micros = duration_since_epoch.as_micros();
            }
            Err(err) => tracing::error!(
                "SharedMemoryWriter writer_id {} failed getting duration for UNIX epoch: {}",
                self.writer_id, err
            ),
        }
        timestamp_micros
    }

    fn end_bench(&self, write_start: SystemTime) -> Duration {
        let write_end = SystemTime::now();
        let write_duration = write_end.duration_since(write_start).unwrap();
        write_duration
    }
}
