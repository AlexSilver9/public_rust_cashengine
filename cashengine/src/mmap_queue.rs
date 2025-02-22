use std::fmt::Write;
use std::fs::File;
use std::path::PathBuf;
use std::ptr::write_bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use memmap2::{MmapMut, MmapOptions};

fn open(file_name: &PathBuf) -> File {
    println!("Creating IPC file {}", file_name.as_os_str().to_string_lossy());
    let open_result = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&file_name);
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
        MmapOptions::new()
            .offset(0)
            .len(file_size)
            .map_mut(file)
            .expect("Failed to map file")
    }
}

fn initialize_mapped_memory(mmap: &mut MmapMut, file_size: usize) -> *mut u8 {
    println!("Initializing memory mapped file with zeros");
    let start_ptr = mmap.as_mut_ptr();
    unsafe {
        write_bytes(start_ptr.offset(0), 0u8, file_size);
    }
    start_ptr
}

pub fn create_log_file(log_file_path: &str) -> File {
    println!("Creating memory logfile at {}", log_file_path);
    let log_file = File::create(log_file_path);
    let log_file = match log_file {
        Ok(file) => file,
        Err(e) => {
            panic!("Failed to create log file: {}", e);
        }
    };
    log_file
}

#[derive(Copy, Clone)]
pub struct ShareablePtr(pub(crate) *mut u8);

// SAFETY: We never alias data when writing from multiple threads.
// Writer threads finish before unmapping.
unsafe impl Send for ShareablePtr {
    // The `ShareablePtr` is not aliased by any other thread.
    // This ensures that no data race occurs when accessing the `start_ptr` in multiple threads.
}

pub fn initialize(mmap_file_path: &str, file_size: usize) -> (ShareablePtr, MmapMut, File) {
    let mmap_file_name = {
        PathBuf::from(mmap_file_path)
    };

    let mmap_file = open(&mmap_file_name);

    resize(&mmap_file, file_size);
    let mut mmap = map_file_to_memory(&mmap_file, file_size);
    let start_ptr = initialize_mapped_memory(&mut mmap, file_size);
    (ShareablePtr(start_ptr),  mmap, mmap_file)
}



pub fn write(id: usize, writers_count: usize, chunk_size: usize, file_size: usize, start_ptr: &ShareablePtr) {
    println!("Starting IPC Writer Thread for Id {}", id);
    let start_ptr = start_ptr;
    let start_ptr: *mut u8 = start_ptr.0;

    let mut buffer = String::with_capacity(chunk_size);
    let mut offset = chunk_size * id;

    println!("IPC Writer Thread Id {} starting at offset {}", id, offset);
    if offset + chunk_size > file_size {
        offset = chunk_size * id;
    }
    while offset + chunk_size <= file_size {
        write_shm(id, writers_count, chunk_size, start_ptr, &mut buffer, offset);
    }

    // Make writes visible for main thread
    // It is not necessary when using `std::thread::scope` but may be necessary in your case.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
    //std::thread::sleep(std::time::Duration::from_millis(1));
}

fn write_shm(id: usize, writers_count: usize, chunk_size: usize, start_ptr: *mut u8, mut buffer: &mut String, mut offset: usize) {
    if offset + chunk_size <= file_size {
        buffer.clear();

        let current_system_time = SystemTime::now();
        let mut micro_timestamp = 0;
        match current_system_time.duration_since(UNIX_EPOCH) {
            Ok(duration_since_epoch) => {
                micro_timestamp = duration_since_epoch.as_micros();
            }
            Err(err) => println!("IPC Writer Thread Id {} failed getting duration for UNIX epoch: {}", id, err),
        }

        write!(&mut buffer, "{}:{}{:11}\n", id, micro_timestamp, offset).unwrap();
        if buffer.len() > chunk_size {
            panic!("IPC Writer Thread Id {} buffer size {} is longer than chunk size: {}", id, buffer.len(), chunk_size);
        }
        println!("IPC Writer Thread Id {} wrote at offset {} at time {}", id, offset, micro_timestamp);

        let write_start = SystemTime::now();
        unsafe {
            // SAFETY: We never overlap on writes.
            // Pointer is living because we using scoped threads.
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr(),
                start_ptr.add(offset),
                chunk_size,
            );
        }
        let write_end = SystemTime::now();
        let write_duration = write_end.duration_since(write_start).unwrap();
        println!("IPC Writer Thread Id {} wrote at offset {} at time {}. Write took {} μs", id, offset, micro_timestamp, write_duration.as_micros());

        // Add offset for ourselves and for odd thread.
        offset += writers_count * chunk_size;
        println!("IPC Writer Thread Id {} incremented to offset {}", id, offset);
        }
}

pub fn read(writer_threads: usize, chunk_size: usize, total_size: usize, start_ptr: &ShareablePtr, log_file: &mut File) {
    let start_ptr = start_ptr;
    let start_ptr: *mut u8 = start_ptr.0;

    let mut offsets = vec![0; writer_threads];
    let value = vec![0u8; chunk_size];

    for i in 0..writer_threads {
        offsets[i] = i * chunk_size;
    }

    loop {
        let mut read_counts = vec![0; writer_threads];
        for id in 0..writer_threads {
            // Make writes visible for main thread
            // It is not necessary when using `std::thread::scope` but may be necessary in your case.
            std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
            if offsets[id] + chunk_size > total_size {
                offsets[id] = chunk_size * id;
            }

            let read_start = SystemTime::now();
            unsafe {
                // SAFETY: We never overlap on writes.
                // Pointer is living because we using scoped threads.
                std::ptr::copy_nonoverlapping(
                    start_ptr.add(offsets[id]),
                    value.as_ptr().cast_mut(),
                    chunk_size,
                );
            }
            read_counts[id] += 1;
            let read_end = SystemTime::now();
            let read_duration = read_end.duration_since(read_start).unwrap();
            println!("IPC Reader Thread offset {} has been read {} times, read in {} μs", offsets[id], read_counts[id], read_duration.as_micros());


            let parse_start = SystemTime::now();
            let value = String::from_utf8(value.to_vec()).unwrap().trim().to_string();
            match value.find(':') {
                Some(index) => {
                    println!("IPC Reader Thread reading Id {}, offset {}, value: {}", id, offsets[id], value);
                    match value.rfind(' ') {
                        Some(start_index) => {
                            match value.rfind('\n') {
                                Some(end_index) => {
                                    let offset = String::from(&value[start_index + 1..end_index]);
                                    let offset: usize = match offset.parse() {
                                        Ok(num) => num,
                                        Err(e) => {
                                            eprintln!("IPC Reader Thread {} failed to parse offset: {}, error: {}", id, offset, e);
                                            continue; // or handle the error in another way
                                        }
                                    };
                                    assert_eq!(offset, offsets[id]);
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
                            eprintln!("IPC Reader Thread {} failed to parse timestamp: {}, error: {}", id, timestamp_str, e);
                            continue;
                        }
                    };

                    let current_system_time = SystemTime::now();
                    match current_system_time.duration_since(UNIX_EPOCH) {
                        Ok(duration_since_epoch) => {
                            let micro_seconds_timestamp = duration_since_epoch.as_micros();
                            let latency = micro_seconds_timestamp - timestamp;
                            println!("IPC Reader Thread {} write time: {}, Read time: {}, Latency: {} μs", id, timestamp, micro_seconds_timestamp, latency);
                        },
                        Err(err) => println!("IPC Reader Thread {} failed getting duration for UNIX epoch: {}", id, err),
                    }

                    let write_result = log_file.write_all(value.as_bytes());
                    match write_result {
                        Ok(_) => {
                            match log_file.write_all(b"\n") {
                                Ok(_) => {},
                                Err(e) => println!("IPC Reader Thread {} failed to write newline to log file: {}", id, e),
                            }
                            match log_file.flush() {
                                Ok(_) => {},
                                Err(e) => println!("IPC Reader Thread {} failed to flush log file: {}", id, e),
                            }
                        },
                        Err(e) => println!("IPC Reader Thread {} failed to write to log file: {}", id, e),
                    }
                    offsets[id] += writer_threads * chunk_size;
                    println!("IPC Reader Thread {} incremented to offset {}", id, offsets[id]);
                },
                None => {
                    //println!("Reader Id {} remains at offset {}", i, offsets[i]);
                    //println!("No index to poll");
                }
            }
            let parse_end = SystemTime::now();
            let parse_duration = parse_end.duration_since(parse_start).unwrap();
            println!("IPC Reader Thread {} string parsing took {} μs", id, parse_duration.as_micros());
        }
    }
}

pub fn close(mmap: MmapMut, mmap_file: File) {
    // Make writes visible for main thread
    // It is not necessary when using `std::thread::scope` but may be necessary in your case.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);

    mmap.flush().expect("IPC Reader Thread failed to flush memory mapped file");
    drop(mmap);
    drop(mmap_file);

    println!("IPC Reader Thread finished");
}