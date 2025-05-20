use std::{
    collections::{HashSet, VecDeque},
    fmt::{Display, Formatter, Result},
};

use crate::link_map::{LinkMap, LinkMapValue};

use super::{
    process_heap::{Process, ProcessHeap},
    WorkerResult,
};

pub struct Trace {
    link_map: LinkMap,
    seen: HashSet<String>,
    heap: ProcessHeap,
    processors: VecDeque<WorkerResult>,
}

impl Trace {
    pub fn new(root: &str, worker_pool_size: u16) -> Self {
        Trace {
            link_map: LinkMap::new(root.to_string()),
            seen: HashSet::from([root.to_string()]),
            heap: ProcessHeap::new(),
            processors: VecDeque::with_capacity(worker_pool_size as usize),
        }
    }

    pub fn get_result(&self) -> LinkMap {
        self.link_map.clone()
    }

    pub fn push_processor(&mut self, worker_res: WorkerResult) {
        self.processors.push_front(worker_res);
    }

    pub fn get_next_processor(&mut self) -> Option<WorkerResult> {
        self.processors.pop_front()
    }

    pub fn get_next_process(&mut self) -> Option<Process> {
        self.heap.pop()
    }

    pub fn queue_to_process(&mut self, url: &str, retry: u8, initial_retry_delay_ms: &u16) {
        if retry == 0 {
            if self.seen.contains(url) {
                return;
            }
            self.seen.insert(url.to_string());
        }
        self.heap
            .push(Process::new(&url, retry, initial_retry_delay_ms));
    }

    pub fn add_result(&mut self, url: &str, result: LinkMapValue) {
        self.link_map.add(url.to_string(), result);
    }

    pub fn has_process_capacity(&self) -> bool {
        self.processors.len() < self.processors.capacity()
    }

    fn progress_bar(&self) -> String {
        let completed = &(self.link_map.map.len() as f32);
        let total = &(self.seen.len() as f32);
        let percentage = ((completed / total) * 100f32).round() as u32;
        let mut bar = String::new();
        for i in 0..100 {
            bar += if i < percentage { "█" } else { " " }
        }
        bar += &format!(
            " | {}/{} ... {} queued, {} in processing",
            completed,
            total,
            &self.heap.len(),
            &self.processors.len()
        );
        bar
    }

    pub fn get_status(&self) -> String {
        format!(
            "\nTracing - {}\n{}",
            self.link_map.root,
            self.progress_bar()
        )
    }
}

impl Display for Trace {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", &self.get_status())
    }
}
