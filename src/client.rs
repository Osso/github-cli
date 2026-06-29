#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Result, bail};
use std::future::Future;
use std::time::Duration;

const GET_RETRY_ATTEMPTS: usize = 3;
const GET_RETRY_DELAY: Duration = Duration::from_secs(2);

pub struct Client {
    pub http: reqwest::Client,
}

impl Client {
    pub fn new(token: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}").parse()?,
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github+json".parse()?,
        );
        headers.insert("X-GitHub-Api-Version", "2022-11-28".parse()?);
        headers.insert(reqwest::header::USER_AGENT, "github-cli/0.1.0".parse()?);

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self { http })
    }

    async fn send(&self, method: reqwest::Method, path: &str) -> Result<reqwest::Response> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.request(method.clone(), &url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("{} {path} failed ({status}): {body}", method);
        }
        Ok(resp)
    }

    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let url = format!("https://api.github.com{path}");
        let resp = self
            .http
            .request(method.clone(), &url)
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("{} {path} failed ({status}): {body}", method);
        }
        Ok(resp)
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        self.get_with_retry(path, parse_json_response).await
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        Ok(self
            .send_json(reqwest::Method::POST, path, body)
            .await?
            .json()
            .await?)
    }

    /// POST that expects no response body (e.g. 202 Cancel, 201 Rerun).
    pub async fn post_empty(&self, path: &str) -> Result<()> {
        self.send(reqwest::Method::POST, path).await?;
        Ok(())
    }

    pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let resp = self.send_json(reqwest::Method::PUT, path, body).await?;
        let text = resp.text().await?;
        if text.is_empty() {
            Ok(serde_json::json!({}))
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.send(reqwest::Method::DELETE, path).await?;
        Ok(())
    }

    /// GET that follows redirects and returns the raw bytes (for log downloads).
    pub async fn get_bytes(&self, path: &str) -> Result<bytes::Bytes> {
        self.get_with_retry(path, parse_bytes_response).await
    }

    async fn get_with_retry<T, F, Fut>(&self, path: &str, parse_response: F) -> Result<T>
    where
        F: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        retry_transient(
            || async {
                let response = self.send(reqwest::Method::GET, path).await?;
                parse_response(response).await
            },
            GET_RETRY_ATTEMPTS,
            GET_RETRY_DELAY,
            is_transient_request_error,
        )
        .await
    }

    pub async fn search_code(
        &self,
        query: &str,
        limit: u32,
        page: u32,
    ) -> Result<serde_json::Value> {
        let q = urlencoding::encode(query);
        let url = format!("https://api.github.com/search/code?q={q}&per_page={limit}&page={page}");
        retry_transient(
            || async {
                let resp = self
                    .http
                    .get(&url)
                    .header(
                        reqwest::header::ACCEPT,
                        "application/vnd.github.text-match+json",
                    )
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await?;
                    bail!("GET /search/code failed ({status}): {body}");
                }
                Ok(resp.json().await?)
            },
            GET_RETRY_ATTEMPTS,
            GET_RETRY_DELAY,
            is_transient_request_error,
        )
        .await
    }
}

async fn parse_json_response(response: reqwest::Response) -> Result<serde_json::Value> {
    Ok(response.json().await?)
}

async fn parse_bytes_response(response: reqwest::Response) -> Result<bytes::Bytes> {
    Ok(response.bytes().await?)
}

async fn retry_transient<T, E, F, Fut, ShouldRetry>(
    mut operation: F,
    max_attempts: usize,
    delay: Duration,
    should_retry: ShouldRetry,
) -> std::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, E>>,
    ShouldRetry: Fn(&E) -> bool,
{
    let mut attempt = 1;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) if attempt < max_attempts && should_retry(&error) => {
                attempt += 1;
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient_request_error(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(is_transient_reqwest_error)
}

fn is_transient_reqwest_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request() || error.is_body()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test]
    async fn retry_transient_retries_until_operation_succeeds() {
        let attempts = Cell::new(0);
        let result = retry_transient(
            || async {
                attempts.set(attempts.get() + 1);
                if attempts.get() < 3 {
                    return Err("temporary");
                }
                Ok("done")
            },
            3,
            std::time::Duration::ZERO,
            |error| *error == "temporary",
        )
        .await;

        assert_eq!(result, Ok("done"));
        assert_eq!(attempts.get(), 3);
    }

    #[tokio::test]
    async fn retry_transient_stops_on_non_retryable_error() {
        let attempts = Cell::new(0);
        let result = retry_transient(
            || async {
                attempts.set(attempts.get() + 1);
                Err::<(), _>("permanent")
            },
            3,
            std::time::Duration::ZERO,
            |error| *error == "temporary",
        )
        .await;

        assert_eq!(result, Err("permanent"));
        assert_eq!(attempts.get(), 1);
    }
}
