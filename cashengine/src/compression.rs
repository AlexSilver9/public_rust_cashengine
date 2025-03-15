use flate2::read::MultiGzDecoder;
use std::io;
use std::io::Read;

pub fn gz_inflate_to_buffer(bytes: &Vec<u8>, buffer: &mut [u8]) -> io::Result<usize> {
    let mut gz = MultiGzDecoder::new(&bytes[..]);
    gz.read(buffer)
}