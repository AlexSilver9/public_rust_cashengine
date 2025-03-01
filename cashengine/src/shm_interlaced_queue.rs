use memmap2::{MmapMut, MmapOptions};
use std::fmt::Write;
use std::fs::File;
use std::path::PathBuf;
use std::ptr::write_bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec;

const MAX_USIZE_STRING_LENGTH: usize = {
    const fn num_digits(mut n: usize) -> usize {
        let mut count = 0;
        while n > 0 {
            n /= 10;
            count += 1;
        }
        count
    }
    num_digits(usize::MAX)
};

#[derive(Copy, Clone)]
pub struct ShareablePtr(pub(crate) *mut u8);

// SAFETY: We never alias data when writing from multiple threads.
// Writer threads finish before unmapping.
unsafe impl Send for ShareablePtr {
    // The `ShareablePtr` is not aliased by any other thread.
    // This ensures that no data race occurs when accessing the `start_ptr` in multiple threads.
}

pub struct SharedMemoryQueue {
    sequence: usize,
    mmap_file: File,
    mmap: MmapMut,
    writers_count: usize,
    log_file: File,
    chunk_size: usize,
    file_size: usize,
    read_buffer: Vec<u8>,
    shareable_ptr: ShareablePtr,
    write_buffers: Vec<String>,
    offsets: Vec<usize>,
    current_writer_id: usize,
}

impl SharedMemoryQueue {

    pub fn create(
        mmap_file_path: &str,
        file_size: usize,
        log_file_path: &str,
        writers_count: usize,
        chunk_size: usize
    ) -> SharedMemoryQueue {
        let mmap_file = SharedMemoryQueue::open(&mmap_file_path);
        SharedMemoryQueue::resize(&mmap_file, file_size);
        let mut mmap = SharedMemoryQueue::map_file_to_memory(&mmap_file, file_size);
        let start_ptr = SharedMemoryQueue::initialize_mapped_memory(&mut mmap, file_size);
        let shareable_ptr = ShareablePtr(start_ptr);
        let log_file = SharedMemoryQueue::create_log_file(log_file_path);
        let mut shm_queue = SharedMemoryQueue {
            sequence: 0,
            mmap_file,
            mmap,
            writers_count,
            log_file,
            chunk_size,
            file_size,
            read_buffer: vec![0u8; chunk_size],
            shareable_ptr,
            write_buffers: vec![String::with_capacity(chunk_size); writers_count],
            offsets: vec![0usize; writers_count],
            current_writer_id: 0,
        };
        shm_queue.initialize_offsets(writers_count);
        shm_queue
    }

    fn initialize_offsets(&mut self, writers_count: usize) {
        for writer_id in 0..writers_count {
            self.offsets[writer_id] = self.chunk_size * writer_id;
            println!("SharedMemoryQueue writer_id {} initialized with offset {}", writer_id, self.offsets[writer_id]);
        }
    }

    fn open(file_path: &str) -> File {
        println!("Creating IPC file {}", file_path);
        let path_buf= PathBuf::from(file_path);
        let open_result = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path_buf);
        let file: File = match open_result {
            Ok(file) => file,
            Err(e) => {
                panic!("Failed to create IPC file: {}", e);
            }
        };
        file
    }

    fn resize(file: &File, file_size: usize) {
        println!("Resizing IPC file to {} bytes", file_size);
        match file.set_len(file_size as u64) {
            Ok(_) => (),
            Err(e) => {
                panic!("Failed to resize IPC file: {}", e);
            }
        }
    }

    fn map_file_to_memory(file: &File, file_size: usize) -> MmapMut {
        println!("Mapping file to memory");
        unsafe {
            match MmapOptions::new()
                .offset(0)
                .len(file_size)
                .map_mut(file) {
                Ok(mmap) => mmap,
                Err(e) => {
                    panic!("Failed to map IPC file to memory: {}", e);
                }
            }
        }
    }

    fn initialize_mapped_memory(mmap: &mut MmapMut, file_size: usize) -> *mut u8 {
        println!("Initializing IPC file with zeros");
        let start_ptr = mmap.as_mut_ptr();
        unsafe {
            write_bytes(start_ptr.offset(0), 0u8, file_size);
        }
        start_ptr
    }

    fn create_log_file(log_file_path: &str) -> File {
        println!("Creating IPC logfile at {}", log_file_path);
        let log_file = File::create(log_file_path);
        let log_file = match log_file {
            Ok(file) => file,
            Err(e) => {
                panic!("Failed to create IPC logfile: {}", e);
            }
        };
        log_file
    }

    pub fn get_read_buffer(&mut self) -> &[u8] {
        &self.read_buffer[..self.chunk_size]
    }

    pub fn write(&mut self, writer_id: usize, message: &[u8]) {
        if self.offsets[writer_id] + self.chunk_size > self.file_size {
            self.offsets[writer_id] = self.chunk_size * writer_id;
        }
        let start_ptr: *mut u8 = self.shareable_ptr.0;
        if self.offsets[writer_id] + self.chunk_size <= self.file_size {
            self.write_buffers[writer_id].clear();

            let current_system_time = SystemTime::now();
            let mut micro_timestamp = 0;
            match current_system_time.duration_since(UNIX_EPOCH) {
                Ok(duration_since_epoch) => {
                    micro_timestamp = duration_since_epoch.as_micros();
                }
                Err(err) => println!("SharedMemoryQueue writer_id {} failed getting duration for UNIX epoch: {}", writer_id, err),
            }

            let message = String::from_utf8(message.to_vec()).unwrap(); // TODO: String and avoid write!(), use copy_nonoverlapping() from below directly
            write!(&mut self.write_buffers[writer_id], "{}:{}:{}:{:0width$}:{}",
                   self.sequence, writer_id, micro_timestamp, self.offsets[writer_id], message, width = MAX_USIZE_STRING_LENGTH).unwrap();
            self.sequence += 1;
            if self.write_buffers[writer_id].len() > self.chunk_size {
                panic!("SharedMemoryQueue writer_id {} buffer size {} is longer than chunk size: {}", writer_id, self.write_buffers[writer_id].len(), self.chunk_size);
            }
            println!("SharedMemoryQueue writer_id {} starts writing at offset {} at time {}", writer_id, self.offsets[writer_id], micro_timestamp);

            let write_start = SystemTime::now();
            unsafe {
                // SAFETY: We never overlap on writes.
                // Pointer is living because we using scoped threads.
                std::ptr::copy_nonoverlapping(
                    self.write_buffers[writer_id].as_ptr(),
                    start_ptr.add(self.offsets[writer_id]),
                    self.chunk_size,
                );
            }
            let write_end = SystemTime::now();
            let write_duration = write_end.duration_since(write_start).unwrap();
            println!("SharedMemoryQueue writer_id {} wrote at offset {} at time {}. Write took {} μs", writer_id, self.offsets[writer_id], micro_timestamp, write_duration.as_micros());

            // Add offset for ourselves and for odd thread.
            self.offsets[writer_id] += self.writers_count * self.chunk_size;
            println!("SharedMemoryQueue writer_id {} incremented to offset {}", writer_id, self.offsets[writer_id]);

            // Make writes visible for main thread
            // It is not necessary when using `std::thread::scope` but may be necessary in your case.
            std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
        }
    }

    fn next_writer_id(&mut self) {
        self.current_writer_id += 1usize;
        if self.current_writer_id >= self.writers_count {
            self.current_writer_id = 0usize;
        }
    }

    pub fn read_next_message(&mut self) {
        let start_ptr: *mut u8 = self.shareable_ptr.0;

        let mut read_counts = vec![0; self.writers_count];

        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        if self.offsets[self.current_writer_id] + self.chunk_size > self.file_size {
            self.offsets[self.current_writer_id] = self.chunk_size * self.current_writer_id;
        }

        let read_start = SystemTime::now();
        unsafe {
            // SAFETY: We never overlap on writes.
            // Pointer is living because we using scoped threads.
            std::ptr::copy_nonoverlapping(
                start_ptr.add(self.offsets[self.current_writer_id]),
                self.read_buffer.as_ptr().cast_mut(),
                self.chunk_size,
            );
        }
        read_counts[self.current_writer_id] += 1;
        let read_end = SystemTime::now();
        let read_duration = read_end.duration_since(read_start).unwrap();
        println!("SharedMemoryQueue read writer_id {} offset {} has been read {} times, read in {} μs",
                 self.current_writer_id, self.offsets[self.current_writer_id], read_counts[self.current_writer_id], read_duration.as_micros());


        let parse_start = SystemTime::now();
        let value = String::from_utf8(self.read_buffer.to_vec()).unwrap().trim().to_string();
        match value.find(':') {
            Some(index) => {
                println!("SharedMemoryQueue reading writer_id {}, offset {}, value: {}",
                         self.current_writer_id, self.offsets[self.current_writer_id], value);
                match value.rfind(' ') {
                    Some(start_index) => {
                        match value.rfind('\n') {
                            Some(end_index) => {
                                let offset = String::from(&value[start_index + 1..end_index]);
                                let offset: usize = match offset.parse() {
                                    Ok(num) => num,
                                    Err(e) => {
                                        eprintln!("SharedMemoryQueue read writer_id {} failed to parse offset: {}, error: {}", self.current_writer_id, offset, e);
                                        0
                                    }
                                };
                                assert_eq!(offset, self.offsets[self.current_writer_id]);
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
                        eprintln!("SharedMemoryQueue read writer_id {} failed to parse timestamp: {}, error: {}", self.current_writer_id, timestamp_str, e);
                        0
                    }
                };

                let current_system_time = SystemTime::now();
                match current_system_time.duration_since(UNIX_EPOCH) {
                    Ok(duration_since_epoch) => {
                        let micro_seconds_timestamp = duration_since_epoch.as_micros();
                        let latency = micro_seconds_timestamp - timestamp;
                        println!("SharedMemoryQueue read writer_id {} write time: {}, Read time: {}, Latency: {} μs", self.current_writer_id, timestamp, micro_seconds_timestamp, latency);
                    },
                    Err(err) => println!("SharedMemoryQueue read writer_id {} failed getting duration for UNIX epoch: {}", self.current_writer_id, err),
                }

                let write_result = self.log_file.write_all(value.as_bytes());
                match write_result {
                    Ok(_) => {
                        match self.log_file.write_all(b"\n") {
                            Ok(_) => {},
                            Err(e) => println!("SharedMemoryQueue read writer_id {} failed to write newline to log file: {}", self.current_writer_id, e),
                        }
                        match self.log_file.flush() {
                            Ok(_) => {},
                            Err(e) => println!("SharedMemoryQueue read writer_id {} failed to flush log file: {}", self.current_writer_id, e),
                        }
                    },
                    Err(e) => println!("SharedMemoryQueue read writer_id {} failed to write to log file: {}", self.current_writer_id, e),
                }
                self.offsets[self.current_writer_id] += self.writers_count * self.chunk_size;
                println!("SharedMemoryQueue read writer_id {} incremented to offset {}", self.current_writer_id, self.offsets[self.current_writer_id]);
            },
            None => {
                //println!("Reader id {} remains at offset {}", i, offsets[i]);
                //println!("No index to poll");
            }
        }
        let parse_end = SystemTime::now();
        let parse_duration = parse_end.duration_since(parse_start).unwrap();
        println!("SharedMemoryQueue read writer_id {} string parsing duration: {} μs", self.current_writer_id, parse_duration.as_micros());

        self.next_writer_id();
    }

    pub fn read_next_messages_for_all_writers(&mut self) -> Vec<Vec<u8>> {
        let start_ptr: *mut u8 = self.shareable_ptr.0;
        let mut read_counts = vec![0; self.writers_count];
        let mut messages = vec![vec![0u8; self.chunk_size]; self.writers_count];

        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        for writer_id in 0..self.writers_count {

            if self.offsets[writer_id] + self.chunk_size > self.file_size {
                self.offsets[writer_id] = self.chunk_size * writer_id;
            }

            let read_start = SystemTime::now();
            unsafe {
                // SAFETY: We never overlap on writes.
                // Pointer is living because we using scoped threads.
                std::ptr::copy_nonoverlapping(
                    start_ptr.add(self.offsets[writer_id]),
                    messages[writer_id].as_ptr().cast_mut(),
                    self.chunk_size,
                );
            }

            read_counts[writer_id] += 1;
            let read_end = SystemTime::now();
            let read_duration = read_end.duration_since(read_start).unwrap();
            println!("SharedMemoryQueue read writer_id {} offset {} has been read {} times, read in {} μs",
                     writer_id, self.offsets[writer_id], read_counts[writer_id], read_duration.as_micros());


            let parse_start = SystemTime::now();
            let value = String::from_utf8(self.read_buffer.to_vec()).unwrap().trim().to_string();
            match value.find(':') {
                Some(index) => {
                    println!("SharedMemoryQueue read writer_id {}, offset {}, value: {}", writer_id, self.offsets[writer_id], value);
                    match value.rfind(' ') {
                        Some(start_index) => {
                            match value.rfind('\n') {
                                Some(end_index) => {
                                    let offset = String::from(&value[start_index + 1..end_index]);
                                    let offset: usize = match offset.parse() {
                                        Ok(num) => num,
                                        Err(e) => {
                                            eprintln!("SharedMemoryQueue read writer_id {} failed to parse offset: {}, error: {}", writer_id, offset, e);
                                            continue; // or handle the error in another way
                                        }
                                    };
                                    assert_eq!(offset, self.offsets[writer_id]);
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
                            eprintln!("SharedMemoryQueue read writer_id {} failed to parse timestamp: {}, error: {}", writer_id, timestamp_str, e);
                            continue;
                        }
                    };

                    let current_system_time = SystemTime::now();
                    match current_system_time.duration_since(UNIX_EPOCH) {
                        Ok(duration_since_epoch) => {
                            let micro_seconds_timestamp = duration_since_epoch.as_micros();
                            let latency = micro_seconds_timestamp - timestamp;
                            println!("SharedMemoryQueue read writer_id {} write time: {}, Read time: {}, Latency: {} μs", writer_id, timestamp, micro_seconds_timestamp, latency);
                        },
                        Err(err) => println!("SharedMemoryQueue read writer_id {} failed getting duration for UNIX epoch: {}", writer_id, err),
                    }

                    let write_result = self.log_file.write_all(value.as_bytes());
                    match write_result {
                        Ok(_) => {
                            match self.log_file.write_all(b"\n") {
                                Ok(_) => {},
                                Err(e) => println!("SharedMemoryQueue read writer_id {} failed to write newline to log file: {}", writer_id, e),
                            }
                            match self.log_file.flush() {
                                Ok(_) => {},
                                Err(e) => println!("SharedMemoryQueue read writer_id {} failed to flush log file: {}", writer_id, e),
                            }
                        },
                        Err(e) => println!("SharedMemoryQueue read writer_id {} failed to write to log file: {}", writer_id, e),
                    }
                    self.offsets[writer_id] += self.writers_count * self.chunk_size;
                    println!("SharedMemoryQueue read writer_id {} incremented to offset {}", writer_id, self.offsets[writer_id]);
                },
                None => {
                    //println!("Reader id {} remains at offset {}", i, offsets[i]);
                    //println!("No index to poll");
                }
            }
            let parse_end = SystemTime::now();
            let parse_duration = parse_end.duration_since(parse_start).unwrap();
            println!("SharedMemoryQueue read writer_id {} string parsing took {} μs", writer_id, parse_duration.as_micros());
        }
        messages
    }

    pub fn close(&self) {
        // Make writes visible for main thread
        // It is not necessary when using `std::thread::scope` but may be necessary in your case.
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        self.mmap.flush().expect("IPC Reader Thread failed to flush memory mapped file");
        println!("IPC Reader Thread finished");
    }
}







