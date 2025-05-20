use tokio::task::JoinHandle;
use tokio::time::sleep;
use trace::Trace;
use tracing::Instrument;

mod process_heap;
mod trace;

use crate::link_gatherer::LinkGatherer;
use crate::link_map::{LinkMap, LinkMapValue};
use std::time::Duration;

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
    pub initial_retry_delay_ms: u16,
    pub max_retries: u8,
}

pub type WorkerResult = JoinHandle<(String, LinkMapValue, u8)>;

impl<T: LinkGatherer + Clone + 'static> SiteTracer<T> {
    #[tracing::instrument(skip_all)]
    fn worker(&self, url_: &str, root_: &str, retry: u8, delay: Option<Duration>) -> WorkerResult {
        let mut link_getter = self.link_getter.clone();
        let url = url_.to_string();
        let root = root_.to_string();
        tokio::spawn(
            async move {
                tracing::info!("Processing URL");
                if let Some(dur) = delay {
                    sleep(dur).await;
                }
                let value = match link_getter.get_links(&url).await {
                    Ok(mut links) => {
                        links.sort();
                        links.dedup();

                        let filtered_links: Vec<String> = links
                            .into_iter()
                            .map(|link| format_link_as_url(&link, &root))
                            .filter(|url| url.starts_with(&root))
                            .collect();
                        tracing::info!("Filtered to {} links", filtered_links.len());
                        tracing::debug!("Filtered Links {:?}", filtered_links);
                        LinkMapValue::Links(filtered_links)
                    }
                    Err(err) => LinkMapValue::Error(err),
                };
                tracing::info!("Finished processing URL");
                (url.to_string(), value, retry + 1)
            }
            .instrument(tracing::info_span!(
                "thread",
                url = url_.to_string(),
                retry = retry,
                delay = format!("{:?}", delay)
            )),
        )
    }

    #[tracing::instrument(skip(self))]
    pub async fn trace(&self, root: &str) -> LinkMap {
        tracing::info!("Begining trace");
        let mut trace = Trace::new(root, self.worker_pool_size);
        trace.push_processor(self.worker(root, root, 0, None));

        print!("\x1B[2J\x1B[H");

        while let Some(result) = trace.get_next_processor() {
            print!("\x1B[f\x1B[0J");
            println!("{}", trace);

            match result.await {
                Ok((url, result, retry)) => match result.clone() {
                    LinkMapValue::Links(links) => {
                        trace.add_result(&url, result);
                        for link in links {
                            trace.queue_to_process(&link, 0, &self.initial_retry_delay_ms);
                        }
                    }
                    LinkMapValue::Error(_) => {
                        if retry > self.max_retries {
                            trace.add_result(&url, result);
                        } else {
                            trace.queue_to_process(&url, retry, &self.initial_retry_delay_ms);
                        }
                    }
                },
                _ => (),
            }
            while trace.has_process_capacity() {
                if let Some(process) = trace.get_next_process() {
                    trace.push_processor(self.worker(
                        &process.url,
                        root,
                        process.retry,
                        process.get_delay(),
                    ));
                } else {
                    break;
                }
            }
        }

        print!("\x1B[f\x1B[0J");
        println!("{}", trace);
        tracing::info!("Finished trace");
        trace.get_result()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        future::Future,
        sync::{Arc, Mutex},
    };

    use crate::{link_gatherer::URLContentGetterError, link_map::LinkMapValue};

    type Response = Result<Vec<String>, URLContentGetterError>;
    #[derive(Debug, Clone)]
    pub enum Responses {
        Always(Response),
        Exhaustable(VecDeque<Response>),
    }

    #[derive(Clone)]
    pub struct MockLG {
        link_map: Arc<Mutex<HashMap<String, Responses>>>,
    }

    impl MockLG {
        pub fn new(link_map: HashMap<String, Responses>) -> Self {
            MockLG {
                link_map: Arc::new(Mutex::new(link_map)),
            }
        }
    }

    impl LinkGatherer for MockLG {
        fn get_links(
            &mut self,
            url: &str,
        ) -> impl Future<Output = Result<Vec<String>, URLContentGetterError>> + Send {
            async {
                if let Some(val) = self.link_map.lock().unwrap().get_mut(url) {
                    return match val {
                        Responses::Always(resp) => match resp {
                            Ok(links) => Ok(links.clone()),
                            Err(err) => Err(err.clone()),
                        },
                        Responses::Exhaustable(ex) => match ex.pop_front() {
                            Some(resp) => match resp {
                                Ok(links) => Ok(links.clone()),
                                Err(err) => Err(err.clone()),
                            },
                            _ => Ok(vec![]),
                        },
                    };
                }
                Ok(vec![])
            }
        }
    }

    use super::*;

    #[tokio::test]
    async fn site_tracer_happy_path() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([
            (
                "http://www.example.com".to_string(),
                Responses::Always(Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ])),
            ),
            (
                "http://www.example.com/two".to_string(),
                Responses::Always(Ok(vec![
                    "http://www.example.com/four".to_string(),
                    "http://www.google.com/six".to_string(),
                    "http://www.example.com/six".to_string(),
                ])),
            ),
            (
                "http://www.example.com/three".to_string(),
                Responses::Always(Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/five".to_string(),
                    "http://www.example.com/seven".to_string(),
                    "http://www.example.com/five".to_string(),
                ])),
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
            max_retries: 4,
            worker_pool_size: 10,
            initial_retry_delay_ms: 250,
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
            Responses::Always(Ok(vec!["/two".to_string(), "three".to_string()])),
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
            max_retries: 1,
            worker_pool_size: 10,
            initial_retry_delay_ms: 25,
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
                Responses::Always(Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ])),
            ),
            (
                "http://www.example.com/two".to_string(),
                Responses::Always(Err(URLContentGetterError::Request(401))),
            ),
            (
                "http://www.example.com/three".to_string(),
                Responses::Always(Err(URLContentGetterError::Content("Oh No".to_string()))),
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
            max_retries: 1,
            worker_pool_size: 10,
            initial_retry_delay_ms: 25,
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
    async fn site_tracer_when_retry_suceeds_returns_links() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([(
            "http://www.example.com".to_string(),
            Responses::Exhaustable(VecDeque::from([
                Err(URLContentGetterError::Request(401)),
                Err(URLContentGetterError::Content(
                    "Mysteries abound".to_string(),
                )),
                Err(URLContentGetterError::Request(401)),
                Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ]),
            ])),
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
            max_retries: 3,
            worker_pool_size: 10,
            initial_retry_delay_ms: 25,
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
    async fn site_tracer_when_max_retries_exhausted_returns_error() {
        let root = "http://www.example.com";

        let mock_lg = MockLG::new(HashMap::from([(
            "http://www.example.com".to_string(),
            Responses::Exhaustable(VecDeque::from([
                Err(URLContentGetterError::Request(401)),
                Err(URLContentGetterError::Request(401)),
                Err(URLContentGetterError::Content(
                    "Mysteries abound".to_string(),
                )),
                Ok(vec![
                    "http://www.example.com/two".to_string(),
                    "http://www.example.com/three".to_string(),
                    "http://www.bolt.example.com/three".to_string(),
                ]),
            ])),
        )]));

        let mut expected = LinkMap::new(root.to_string());
        expected.add(
            "http://www.example.com".to_string(),
            LinkMapValue::Error(URLContentGetterError::Content(
                "Mysteries abound".to_string(),
            )),
        );

        let page = SiteTracer {
            link_getter: mock_lg,
            max_retries: 2,
            worker_pool_size: 10,
            initial_retry_delay_ms: 25,
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
