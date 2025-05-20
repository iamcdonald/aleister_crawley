use std::{cmp::Ordering, collections::BinaryHeap, time::Duration};

use jiff::Timestamp;

#[derive(Eq, PartialOrd)]
pub struct Process {
    pub url: String,
    pub timestamp: Timestamp,
    pub retry: u8,
}

impl Process {
    pub fn new(url: &str, retry: u8, base_delay_ms: &u16) -> Self {
        let timestamp = if retry == 0 {
            Timestamp::now()
        } else {
            Timestamp::now()
                .checked_add(Duration::from_millis(
                    *base_delay_ms as u64 * (2 as u64).pow((retry) as u32),
                ))
                .unwrap()
        };
        Process {
            url: url.to_string(),
            retry,
            timestamp,
        }
    }

    pub fn get_delay(&self) -> Option<Duration> {
        match Duration::try_from(Timestamp::now().until(self.timestamp).unwrap()) {
            Ok(wait_dur) => {
                if wait_dur > Duration::from_millis(0) {
                    Some(wait_dur)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

impl PartialEq for Process {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}

impl Ord for Process {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

pub struct ProcessHeap(BinaryHeap<Process>);

impl ProcessHeap {
    pub fn new() -> Self {
        ProcessHeap(BinaryHeap::new())
    }

    pub fn push(&mut self, process: Process) {
        self.0.push(process)
    }

    pub fn pop(&mut self) -> Option<Process> {
        self.0.pop()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
