pub const MAX_USIZE_STRING_LENGTH: usize = {
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