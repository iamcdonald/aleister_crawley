use std::future::Future;

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum URLContentGetterError {
    #[error("request error")]
    Request(u16),
    #[error("content error")]
    Content(String),
}

pub trait URLContentGetter {
    fn get_http_response_body(
        &self,
        url: &str,
    ) -> impl Future<Output = Result<String, URLContentGetterError>> + Send;
}

impl URLContentGetter for reqwest::Client {
    #[tracing::instrument(skip(self))]
    fn get_http_response_body(
        &self,
        url: &str,
    ) -> impl Future<Output = Result<String, URLContentGetterError>> + Send {
        async move {
            let url = url.to_string();
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("user-agent", "scrapey/1.0".parse().unwrap());
            match self.get(url).headers(headers).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(content) => Ok(content),
                    Err(err) => {
                        tracing::error!("{}", err.to_string());
                        Err(URLContentGetterError::Content(err.to_string()))
                    }
                },
                Err(err) => {
                    tracing::error!("{}", err.to_string());
                    Err(URLContentGetterError::Request(
                        err.status().and_then(|sc| Some(sc.as_u16())).unwrap_or(0),
                    ))
                }
            }
        }
    }
}
