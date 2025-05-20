use std::future::Future;

use super::{url_content_getter::URLContentGetterError, URLContentGetter};
use scraper::{Html, Selector};

pub trait LinkGatherer: Send + Sync + Clone {
    fn get_links(
        &mut self,
        url: &str,
    ) -> impl Future<Output = Result<Vec<String>, URLContentGetterError>> + Send;
}

#[derive(Clone, Debug)]
pub struct Page<T = reqwest::Client> {
    client: T,
}

impl<T: URLContentGetter + Clone> Page<T> {
    pub fn new(client: T) -> Self {
        Page { client }
    }
}

impl<T: URLContentGetter + Clone + Send + Sync> LinkGatherer for Page<T> {
    #[tracing::instrument(skip(self))]
    fn get_links(
        &mut self,
        url: &str,
    ) -> impl Future<Output = Result<Vec<String>, URLContentGetterError>> + Send {
        async move {
            let url = url.to_string();

            match self.client.get_http_response_body(&url).await {
                Ok(text) => {
                    let html = Html::parse_document(&text);
                    let links = html
                        .select(&Selector::parse("a").unwrap())
                        .into_iter()
                        .flat_map(|f| match f.attr("href") {
                            Some(href) => vec![href.to_string()],
                            _ => vec![],
                        })
                        .collect::<Vec<_>>();
                    tracing::info!("Found {} links", links.len());
                    tracing::debug!("Links {:?}", links);
                    Ok(links)
                }
                Err(err) => Err(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::URLContentGetter;
    use crate::link_gatherer::url_content_getter::URLContentGetterError;
    use std::collections::HashMap;

    #[derive(Clone)]
    pub struct MockURLCG {
        map: HashMap<String, Result<String, URLContentGetterError>>,
    }

    impl MockURLCG {
        pub fn new(map: HashMap<String, Result<String, URLContentGetterError>>) -> Self {
            MockURLCG { map }
        }
    }

    impl URLContentGetter for MockURLCG {
        async fn get_http_response_body(&self, url: &str) -> Result<String, URLContentGetterError> {
            match self.map.get(url) {
                Some(x) => match x {
                    Ok(content) => Ok(content.clone()),
                    Err(err) => Err(err.clone()),
                },
                None => Ok("".to_string()),
            }
        }
    }

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[tokio::test]
    async fn link_gatherer_happy_path() {
        let url = "https://example.com";
        let html = r#"
<html>
  <body>
    <a href="https://www.example.com">home</a>
    <a href="https://www.example.com/one">home</a>
    <a href="two">home</a>
    <a href="/three/four?hello=there">home</a>
  </body>
</html>"#;
        let mucg = MockURLCG::new(HashMap::from([(url.to_string(), Ok(html.to_string()))]));
        let mut page = Page::new(mucg);
        let links = page.get_links(url).await;
        assert_eq!(
            links.unwrap(),
            vec![
                "https://www.example.com".to_string(),
                "https://www.example.com/one".to_string(),
                "two".to_string(),
                "/three/four?hello=there".to_string()
            ]
        )
    }

    #[tokio::test]
    async fn link_gatherer_returns_error() {
        let url = "https://example.com";

        let mucg = MockURLCG::new(HashMap::from([(
            url.to_string(),
            Err(URLContentGetterError::Request(404)),
        )]));
        let mut page = Page::new(mucg);
        match page.get_links(url).await {
            Ok(_) => assert!(false, "should throw error"),
            Err(err) => assert_eq!(err, URLContentGetterError::Request(404)),
        }
    }
}
