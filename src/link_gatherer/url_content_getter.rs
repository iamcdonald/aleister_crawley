use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum URLContentGetterError {
    #[error("request error")]
    Request(u16),
    #[error("content error")]
    Content(String),
}

pub trait URLContentGetter {
    async fn get_http_response_body(&self, url: &str) -> Result<String, URLContentGetterError>;
}

impl URLContentGetter for reqwest::Client {
    async fn get_http_response_body(&self, url: &str) -> Result<String, URLContentGetterError> {
        let url = url.to_string();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("user-agent", "scrapey/1.0".parse().unwrap());
        match self.get(url).headers(headers).send().await {
            Ok(resp) => match resp.text().await {
                Ok(content) => Ok(content),
                Err(err) => Err(URLContentGetterError::Content(err.to_string())),
            },
            Err(err) => Err(URLContentGetterError::Request(
                err.status().and_then(|sc| Some(sc.as_u16())).unwrap_or(0),
            )),
        }
    }
}
