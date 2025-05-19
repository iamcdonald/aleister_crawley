use tokio::task::JoinHandle;

use crate::link_gatherer::LinkGatherer;
use crate::link_map::{LinkMap, LinkMapValue};
use std::collections::{HashMap, VecDeque};

fn format_link_as_url(link: &str, root: &str) -> String {
    if link.starts_with("http") {
        link.to_string()
    } else {
        let processed_url = if link.starts_with("/") {
            link
        } else {
            &("/".to_string() + link)
        };
        root.to_string() + processed_url
    }
}

pub struct SiteTracer<T: LinkGatherer + Clone + 'static> {
    pub link_getter: T,
    pub worker_pool_size: u16,
}

impl<T: LinkGatherer + Clone + 'static> SiteTracer<T> {
    fn worker(&self, url_: &str, root_: &str) -> JoinHandle<(String, LinkMapValue)> {
        let mut link_getter = self.link_getter.clone();
        let url = url_.to_string();
        let root = root_.to_string();
        tokio::spawn(async move {
            let value = match link_getter.get_links(&url).await {
                Ok(mut links) => {
                    links.sort();
                    links.dedup();

                    let filtered_links: Vec<String> = links
                        .into_iter()
                        .map(|link| format_link_as_url(&link, &root))
                        .filter(|url| url.starts_with(&root))
                        .collect();
                    LinkMapValue::Links(filtered_links)
                }
                Err(err) => LinkMapValue::Error(err),
            };
            (url.to_string(), value)
        })
    }

    pub async fn trace(&self, root: &str) -> LinkMap {
        let mut link_map = LinkMap::new(root.to_string());
        let mut seen = HashMap::from([(root.to_string(), ())]);
        let mut queue: VecDeque<String> = VecDeque::from([root.to_string()]);
        let mut processors = VecDeque::with_capacity(self.worker_pool_size as usize);
        processors.push_front(self.worker(root, root));

        while let Some(result) = processors.pop_front() {
            // output some progress indication
            print!("\x1B[f\x1B[0J");
            println!(
                "\nScraping Progress\n{} in queue\n{} found links",
                queue.len(),
                seen.len(),
            );
            match result.await {
                Ok((url, result)) => {
                    link_map.add(url, result.clone());
                    if let LinkMapValue::Links(links) = result {
                        for link in links {
                            if seen.contains_key(&link) {
                                continue;
                            }
                            seen.insert(link.clone(), ());
                            queue.push_front(link.clone());
                        }
                    }
                }
                _ => (),
            }
            while processors.len() < (self.worker_pool_size as usize) {
                if let Some(link) = queue.pop_front() {
                    processors.push_back(self.worker(&link, root));
                } else {
                    break;
                }
            }
        }
        link_map
    }
}

#[cfg(test)]
mod tests {
    use crate::{link_gatherer::URLContentGetterError, link_map::LinkMapValue};

    type Response = Result<Vec<String>, URLContentGetterError>;

    #[derive(Clone)]
    pub struct MockLG {
        link_map: HashMap<String, Response>,
    }

    impl MockLG {
        pub fn new(link_map: HashMap<String, Response>) -> Self {
            MockLG { link_map }
        }
    }

    impl LinkGatherer for MockLG {
        async fn get_links(&mut self, url: &str) -> Response {
            if let Some(resp) = self.link_map.get_mut(url) {
                return match resp {
                    Ok(links) => Ok(links.clone()),
                    Err(err) => Err(err.clone()),
                };
            }
            Ok(vec![])
        }
    }

    use super::*;

    #[tokio::test]
    async fn site_tracer_happy_path() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([
            (
                "http://www.example.com".to_string(),
                Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ]),
            ),
            (
                "http://www.example.com/two".to_string(),
                Ok(vec![
                    "http://www.example.com/four".to_string(),
                    "http://www.google.com/six".to_string(),
                    "http://www.example.com/six".to_string(),
                ]),
            ),
            (
                "http://www.example.com/three".to_string(),
                Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/five".to_string(),
                    "http://www.example.com/seven".to_string(),
                    "http://www.example.com/five".to_string(),
                ]),
            ),
        ]));

        let mut expected = LinkMap::new(root.to_string());
        expected.add(
            "http://www.example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://www.example.com/two".to_string(),
                "http://www.example.com/three".to_string(),
            ]),
        );
        expected.add(
            "http://www.example.com/two".to_string(),
            LinkMapValue::Links(vec![
                "http://www.example.com/four".to_string(),
                "http://www.example.com/six".to_string(),
            ]),
        );
        expected.add(
            "http://www.example.com/three".to_string(),
            LinkMapValue::Links(vec![
                "http://www.example.com/two".to_string(),
                "http://www.example.com/five".to_string(),
                "http://www.example.com/seven".to_string(),
            ]),
        );

        let page = SiteTracer {
            link_getter: mock_lg,
            worker_pool_size: 5,
        };

        let link_map = page.trace(root).await;

        for (key, expected) in expected.map {
            match expected {
                LinkMapValue::Links(mut ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Links(mut a) => {
                        a.sort();
                        ex.sort();
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Links value at {}", key),
                },
                LinkMapValue::Error(ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Error(a) => {
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Error value at {}", key),
                },
            }
        }
    }

    #[tokio::test]
    async fn site_tracer_handles_relative_urls() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([(
            "http://www.example.com".to_string(),
            Ok(vec!["/two".to_string(), "three".to_string()]),
        )]));

        let mut expected = LinkMap::new(root.to_string());
        expected.add(
            "http://www.example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://www.example.com/two".to_string(),
                "http://www.example.com/three".to_string(),
            ]),
        );

        let page = SiteTracer {
            link_getter: mock_lg,
            worker_pool_size: 5,
        };
        let link_map = page.trace(root).await;

        for (key, expected) in expected.map {
            match expected {
                LinkMapValue::Links(mut ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Links(mut a) => {
                        a.sort();
                        ex.sort();
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Links value at {}", key),
                },
                LinkMapValue::Error(ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Error(a) => {
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Error value at {}", key),
                },
            }
        }
    }

    #[tokio::test]
    async fn site_tracer_unhappy_path() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([
            (
                "http://www.example.com".to_string(),
                Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ]),
            ),
            (
                "http://www.example.com/two".to_string(),
                Err(URLContentGetterError::Request(401)),
            ),
            (
                "http://www.example.com/three".to_string(),
                Err(URLContentGetterError::Content("Oh No".to_string())),
            ),
        ]));

        let mut expected = LinkMap::new(root.to_string());
        expected.add(
            "http://www.example.com".to_string(),
            LinkMapValue::Links(vec![
                "http://www.example.com/two".to_string(),
                "http://www.example.com/three".to_string(),
            ]),
        );
        expected.add(
            "http://www.example.com/two".to_string(),
            LinkMapValue::Error(URLContentGetterError::Request(401)),
        );
        expected.add(
            "http://www.example.com/three".to_string(),
            LinkMapValue::Error(URLContentGetterError::Content("Oh No".to_string())),
        );

        let page = SiteTracer {
            link_getter: mock_lg,
            worker_pool_size: 5,
        };
        let link_map = page.trace(root).await;

        for (key, expected) in expected.map {
            match expected {
                LinkMapValue::Links(mut ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Links(mut a) => {
                        a.sort();
                        ex.sort();
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Links value at {}", key),
                },
                LinkMapValue::Error(ex) => match link_map.map.get(&key).unwrap().clone() {
                    LinkMapValue::Error(a) => {
                        assert_eq!(a, ex)
                    }
                    _ => assert!(false, "Actual should have Error value at {}", key),
                },
            }
        }
    }
}
