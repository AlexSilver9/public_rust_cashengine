use memmap2::{MmapMut, MmapOptions};
use std::fs::File;
use std::ptr::write_bytes;
use std::time::{Duration, SystemTime};
use std::vec;

#[derive(Copy, Clone)]
pub struct ShareablePtr(pub(crate) *mut u8);

// SAFETY: We never alias data when writing from multiple threads.
// Writer threads finish before unmapping.
unsafe impl Send for ShareablePtr {
    // The `ShareablePtr` is not aliased by any other thread.
    // This ensures that no data race occurs when accessing the `start_ptr` in multiple threads.
}

pub struct SharedMemoryReader<'a> {
    mmap_file: &'a File,
    mmap: MmapMut,
    log_file: File,
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
        log_file_path: &str,
        chunk_size: usize,
        chunk_count: usize,
    ) -> SharedMemoryReader<'a> {
        let file_size = chunk_size * chunk_count;
        let mut mmap = SharedMemoryReader::map_file_to_memory(&mmap_file, file_size);
        let start_ptr =
            SharedMemoryReader::initialize_start_ptr_to_mapped_memory(&mut mmap, file_size);
        let shareable_ptr = ShareablePtr(start_ptr);
        let log_file = SharedMemoryReader::create_log_file(log_file_path);
        let mut shm_reader = SharedMemoryReader {
            mmap_file,
            mmap,
            log_file,
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
        println!("Mapping file to memory");
        unsafe {
            match MmapOptions::new().offset(0).len(file_size).map_mut(file) {
                Ok(mmap) => mmap,
                Err(e) => {
                    panic!("Failed to map SHM file to memory: {}", e);
                }
            }
        }
    }

    fn initialize_start_ptr_to_mapped_memory(mmap: &mut MmapMut, file_size: usize) -> *mut u8 {
        println!("Initializing SHM file with zeros");
        let start_ptr = mmap.as_mut_ptr();
        unsafe {
            write_bytes(start_ptr.offset(0), 0u8, file_size);
        }
        start_ptr
    }

    fn create_log_file(log_file_path: &str) -> File {
        println!("Creating SHM logfile at {}", log_file_path);
        let log_file = File::create(log_file_path);
        let log_file = match log_file {
            Ok(file) => file,
            Err(e) => {
                panic!("Failed to create SHM logfile: {}", e);
            }
        };
        log_file
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
            // Pointer is living because we using scoped threads.
            std::ptr::copy_nonoverlapping(
                start_ptr.wrapping_offset(target_offset as isize),
                self.read_buffer.as_ptr().cast_mut(),
                self.chunk_size,
            );
        }

        let read_duration = self.end_bench(read_start);

        /*println!(
            "SharedMemoryReader read chunk_id {} at offset {} in {} μs",
            self.current_chunk_id,
            target_offset,
            read_duration.as_micros()
        );*/

        let parse_start = SystemTime::now();
        let value = String::from_utf8(self.read_buffer.to_vec())
            .unwrap()
            .trim()
            .to_string();
        /*match value.find(':') {
            Some(index) => {
                println!("SharedMemoryReader read current_chunk_id {} at offset {}, value: {}",
                         self.current_chunk_id, target_offset, value);
                match value.rfind(' ') {
                    Some(start_index) => {
                        match value.rfind('\n') {
                            Some(end_index) => {
                                let offset = String::from(&value[start_index + 1..end_index]);
                                let offset: usize = match offset.parse() {
                                    Ok(num) => num,
                                    Err(e) => {
                                        eprintln!("SharedMemoryReader read chunk_id {} failed to parse offset: {}, error: {}", self.current_chunk_id, offset, e);
                                        0
                                    }
                                };
                                //assert_eq!(offset, self.offsets[self.current_writer_id]);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                use std::io::Write;
                let timestamp_str = match &value.find(' ') {
                    Some(to) => &value[index + 1..*to],
                    None => &value[index + 1..],
                };

                let timestamp: u128 = match timestamp_str.parse() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("SharedMemoryReader read writer_id {} failed to parse timestamp: {}, error: {}", self.current_writer_id, timestamp_str, e);
                        0
                    }
                };

                let current_system_time = SystemTime::now();
                match current_system_time.duration_since(UNIX_EPOCH) {
                    Ok(duration_since_epoch) => {
                        let micro_seconds_timestamp = duration_since_epoch.as_micros();
                        let latency = micro_seconds_timestamp - timestamp;
                        println!("SharedMemoryReader read writer_id {} write time: {}, Read time: {}, Latency: {} μs", self.current_writer_id, timestamp, micro_seconds_timestamp, latency);
                    },
                    Err(err) => println!("SharedMemoryReader read writer_id {} failed getting duration for UNIX epoch: {}", self.current_writer_id, err),
                }

                let write_result = self.log_file.write_all(value.as_bytes());
                match write_result {
                    Ok(_) => {
                        match self.log_file.write_all(b"\n") {
                            Ok(_) => {},
                            Err(e) => println!("SharedMemoryReader read writer_id {} failed to write newline to log file: {}", self.current_writer_id, e),
                        }
                        match self.log_file.flush() {
                            Ok(_) => {},
                            Err(e) => println!("SharedMemoryReader read writer_id {} failed to flush log file: {}", self.current_writer_id, e),
                        }
                    },
                    Err(e) => println!("SharedMemoryReader read writer_id {} failed to write to log file: {}", self.current_writer_id, e),
                }
            },
            None => {
                //println!("Reader id {} remains at offset {}", i, offsets[i]);
                //println!("No index to poll");
            }
        }
         */
        let parse_end = SystemTime::now();
        let parse_duration = parse_end.duration_since(parse_start).unwrap();
        /*println!(
            "SharedMemoryReader with chunk_id {} parsed to string with duration: {} μs",
            self.current_chunk_id,
            parse_duration.as_micros()
        );*/

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
        let read_end = SystemTime::now();
        let read_duration = read_end.duration_since(read_start);
        read_duration.unwrap_or_else(|e| {
            println!("SharedMemoryReader failed getting duration for read: {}", e);
            Duration::new(0, 0)
        })
    }

    fn start_bench(&self) -> SystemTime {
        SystemTime::now()
    }
}
