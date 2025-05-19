use std::{cmp::Ordering, collections::BinaryHeap};

use jiff::Timestamp;

#[derive(Eq, PartialOrd)]
pub struct Process {
    pub url: String,
    pub timestamp: Timestamp,
    pub retry: u8,
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
