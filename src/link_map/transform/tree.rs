use crate::{link_gatherer::URLContentGetterError, link_map::LinkMap, link_map::LinkMapValue};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write;

#[derive(Clone, Debug)]
struct Level(i32);

struct Item {
    url: String,
    level: Level,
    active: Vec<bool>,
    parents: HashMap<String, ()>,
}

struct CountMap(HashMap<String, i32>);

impl CountMap {
    pub fn new() -> Self {
        CountMap(HashMap::new())
    }
    pub fn increment(&mut self, url: &str) {
        match self.0.get_mut(url) {
            Some(val) => {
                *val += 1;
            }
            None => {
                self.0.insert(url.to_string(), 1);
            }
        };
    }
    pub fn decrement(&mut self, url: &str) {
        match self.0.get_mut(url) {
            Some(val) => {
                *val -= 1;
            }
            None => {}
        };
    }

    pub fn processed(&mut self, url: &str) {
        match self.0.get_mut(url) {
            Some(val) => {
                *val = 100;
            }
            None => {}
        };
    }

    pub fn is_queued_for_processing(&mut self, url: &str) -> bool {
        self.0.get(url) > Some(&0)
    }
}

fn get_indent(level: &Level, active_levels: &Vec<bool>, is_tail: bool) -> String {
    match level.0 {
        0 => "".to_string(),
        lev => (1..=lev)
            .map(|x| {
                let mut out = "".to_string();
                if x == lev {
                    if is_tail {
                        out += "├──";
                    } else {
                        out += "└──";
                    }
                } else {
                    match active_levels.get((x - 1) as usize) {
                        Some(active) => match active {
                            true => out += "│  ",
                            _ => out += "   ",
                        },
                        _ => out += "   ",
                    };
                }
                out
            })
            .collect::<String>(),
    }
}

fn get_next_level(dfs: &VecDeque<Item>) -> Level {
    dfs.front()
        .and_then(|i| Some(i.level.clone()))
        .unwrap_or(Level(-1))
}

pub fn to_tree(link_map: &LinkMap) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    let mut visited = CountMap::new();
    let mut dfs: VecDeque<Item> = VecDeque::from([Item {
        url: link_map.root.clone(),
        level: Level(0),
        active: vec![],
        parents: HashMap::new(),
    }]);

    while let Some(Item {
        url,
        level,
        active,
        parents,
    }) = dfs.pop_front()
    {
        visited.decrement(&url);
        let next_level = get_next_level(&dfs);
        let is_tail = level.0 <= next_level.0;
        let mut new_active = Vec::from(active.clone());
        if level.0 > 0 {
            new_active.push(level.0 == next_level.0);
        }

        let mut cycle = "".to_string();
        if parents.contains_key(&url) {
            cycle += " ⟳"
        } else if visited.is_queued_for_processing(&url) {
            cycle += " 🔗"
        } else {
            visited.processed(&url);
            if let Some(LinkMapValue::Links(links)) = link_map.map.get(&url) {
                let mut new_parents = parents.clone();
                new_parents.insert(url.clone(), ());
                for link in links.iter().rev() {
                    visited.increment(&link);
                    dfs.push_front(Item {
                        url: link.clone(),
                        active: new_active.clone(),
                        level: Level(level.0 + 1),
                        parents: new_parents.clone(),
                    })
                }
            }
        }

        let indent = get_indent(&level, &active, is_tail);
        let error = match cycle.is_empty() {
            true => match link_map.map.get(&url) {
                Some(LinkMapValue::Error(err)) => match err {
                    URLContentGetterError::Request(code) => format!(" - 😵 {}", code),
                    URLContentGetterError::Content(text) => format!(" - 😵 \"{}\"", text),
                },
                _ => "".to_string(),
            },
            _ => "".to_string(),
        };

        match write!(output, "{}{}{}{}\n", indent, url, cycle, error) {
            Err(err) => return Err(err),
            _ => (),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use crate::link_gatherer::URLContentGetterError;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn display_simple() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/three".to_string(),
                "http://example.com/four".to_string(),
            ]),
        );
        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/three
│  └──http://example.com/four
└──http://example.com/two
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_handles_overhang() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/three".to_string(),
                "http://example.com/four".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/two".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/five".to_string(),
                "http://example.com/six".to_string(),
            ]),
        );
        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/three
│  └──http://example.com/four
└──http://example.com/two
   ├──http://example.com/five
   └──http://example.com/six
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_deeply_nested() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/three".to_string(),
                "http://example.com/four".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/three".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/five".to_string(),
                "http://example.com/six".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/six".to_string(),
            LinkMapValue::Links(vec!["http://example.com/seven".to_string()]),
        );
        link_map.add(
            "http://example.com/four".to_string(),
            LinkMapValue::Links(vec!["http://example.com/eight".to_string()]),
        );

        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/three
│  │  ├──http://example.com/five
│  │  └──http://example.com/six
│  │     └──http://example.com/seven
│  └──http://example.com/four
│     └──http://example.com/eight
└──http://example.com/two
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_tail() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec!["http://example.com/one".to_string()]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/two".to_string(),
                "http://example.com/t_w_o".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/two".to_string(),
            LinkMapValue::Links(vec!["http://example.com/three".to_string()]),
        );
        link_map.add(
            "http://example.com/three".to_string(),
            LinkMapValue::Links(vec!["http://example.com/four".to_string()]),
        );
        link_map.add(
            "http://example.com/four".to_string(),
            LinkMapValue::Links(vec!["http://example.com/five".to_string()]),
        );

        let expected = r#"http://example.com
└──http://example.com/one
   ├──http://example.com/two
   │  └──http://example.com/three
   │     └──http://example.com/four
   │        └──http://example.com/five
   └──http://example.com/t_w_o
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_shows_cycles() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/three".to_string(),
                "http://example.com".to_string(),
            ]),
        );
        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/three
│  └──http://example.com ⟳
└──http://example.com/two
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_favours_shallower_nesting() {
        // if a url appears nearer the root that url should show the expanded links
        // other references to that url should show the link symbol 🔗
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/two".to_string(),
                "http://example.com".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/two".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com".to_string(),
                "http://example.com/one".to_string(),
            ]),
        );

        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/two 🔗
│  └──http://example.com ⟳
└──http://example.com/two
   ├──http://example.com ⟳
   └──http://example.com/one 🔗
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_with_gap() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/two".to_string(),
                "http://example.com/three".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/three".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com".to_string(),
                "http://example.com/one".to_string(),
            ]),
        );

        let expected = r#"http://example.com
├──http://example.com/one
│  ├──http://example.com/two 🔗
│  └──http://example.com/three
│     ├──http://example.com ⟳
│     └──http://example.com/one ⟳
└──http://example.com/two
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }

    #[test]
    fn display_with_error() {
        let mut link_map = LinkMap::new("http://example.com".to_string());
        link_map.add(
            "http://example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/one".to_string(),
                "http://example.com/two".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/one".to_string(),
            LinkMapValue::Error(URLContentGetterError::Request(401)),
        );
        link_map.add(
            "http://example.com/two".to_string(),
            LinkMapValue::Links(vec![
                "http://example.com/three".to_string(),
                "http://example.com/one".to_string(),
            ]),
        );
        link_map.add(
            "http://example.com/three".to_string(),
            LinkMapValue::Error(URLContentGetterError::Content(
                "something went wrong".to_string(),
            )),
        );

        let expected = r#"http://example.com
├──http://example.com/one - 😵 401
└──http://example.com/two
   ├──http://example.com/three - 😵 "something went wrong"
   └──http://example.com/one 🔗
"#;
        assert_eq!(to_tree(&link_map), Ok(expected.to_string()));
    }
}
