use std::collections::HashMap;

use crate::link_gatherer::URLContentGetterError;

#[derive(Debug, PartialEq, Clone)]
pub enum LinkMapValue {
    Links(Vec<String>),
    Error(URLContentGetterError),
}

#[derive(Debug, PartialEq)]
pub struct LinkMap {
    pub root: String,
    pub map: HashMap<String, LinkMapValue>,
}

impl LinkMap {
    pub fn new(root: String) -> Self {
        LinkMap {
            root,
            map: HashMap::new(),
        }
    }

    pub fn add(&mut self, url: String, value: LinkMapValue) {
        self.map.insert(url, value);
    }
}
