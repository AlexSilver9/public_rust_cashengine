use std::collections::VecDeque;

pub struct P95Tracker {
    data: VecDeque<u128>,
    capacity: usize,
}

impl P95Tracker {
    pub fn new(capacity: usize) -> Self {
        P95Tracker {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: u128) {
        if self.data.len() < self.capacity {
            // If we haven't reached capacity, just insert and sort
            self.data.push_back(value);
            self.data.make_contiguous().sort_unstable();
        } else if value < *self.data.back().unwrap() {
            // If the new value is smaller than the largest in our list
            // remove the largest and insert the new value
            self.data.pop_back();
            let insert_pos = self.data.binary_search(&value).unwrap_or_else(|e| e);
            self.data.insert(insert_pos, value);
        }
    }

    pub fn has_enough_samples(&self) -> bool {
        self.data.len() == self.data.capacity()
    }

    pub fn p95(&self) -> Option<u128> {
        if self.data.is_empty() {
            None
        } else {
            let index = (self.data.len() as f64 * 0.95).ceil() as usize - 1;
            Some(self.data[index])
        }
    }
}
