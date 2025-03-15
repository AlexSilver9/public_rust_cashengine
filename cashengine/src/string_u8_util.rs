pub unsafe fn null_terminated_u8_to_utf8_str_unchecked(utf8_src: &[u8]) -> &str {
    let nul_range_end = utf8_src.iter()
        .position(|&c| c == b'\0')
        .unwrap_or(utf8_src.len()); // default to length if no `\0` present
    std::str::from_utf8_unchecked(&utf8_src[0..nul_range_end])
}