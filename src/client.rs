#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Result, bail};
use std::future::Future;
use std::time::Duration;

const API_BASE_URL: &str = "https://api.github.com";
const GET_RETRY_ATTEMPTS: usize = 3;
const GET_RETRY_DELAY: Duration = Duration::from_secs(2);

pub struct Client {
    pub http: reqwest::Client,
    base_url: String,
}

impl Client {
    pub fn new(token: &str) -> Result<Self> {
        Self::with_base_url(token, API_BASE_URL)
    }

    fn with_base_url(token: &str, base_url: &str) -> Result<Self> {
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

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(token: &str, base_url: &str) -> Result<Self> {
        Self::with_base_url(token, base_url)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
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

    pub(crate) async fn post_json_redacted(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self.endpoint(path);
        let response = self.http.post(url).json(body).send().await?;
        if !response.status().is_success() {
            bail!("POST {path} failed ({})", response.status());
        }
        Ok(response.json().await?)
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_http_fixture(status: &str, body: &str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let body = body.to_owned();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}"), server)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let bytes_read = stream.read(&mut buffer).unwrap();
            if bytes_read == 0 {
                return;
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then_some(value)
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + content_length {
                return;
            }
        }
    }

    #[tokio::test]
    async fn redacted_json_post_omits_response_body_from_display_and_debug() {
        let canary = "https://curseforge.example/upload?token=response-body-canary";
        let (base_url, server) = spawn_http_fixture(
            "422 Unprocessable Entity",
            &format!(r#"{{"message":"{canary}"}}"#),
        );
        let client = Client::for_test("request-token", &base_url).unwrap();
        let body = serde_json::json!({"config": {"url": "https://curseforge.example?token=submitted-json-canary"}});

        let error = client
            .post_json_redacted("/repos/Osso/SpellMeter/hooks", &body)
            .await
            .unwrap_err();
        server.join().unwrap();

        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(
            display.contains("POST /repos/Osso/SpellMeter/hooks failed (422 Unprocessable Entity)")
        );
        assert!(!display.contains(canary));
        assert!(!display.contains("submitted-json-canary"));
        assert!(!debug.contains(canary));
        assert!(!debug.contains("submitted-json-canary"));
    }

    #[tokio::test]
    async fn redacted_json_post_preserves_successful_json_parsing() {
        let (base_url, server) = spawn_http_fixture("201 Created", r#"{"id":1234}"#);
        let client = Client::for_test("request-token", &base_url).unwrap();

        let response = client
            .post_json_redacted("/repos/Osso/SpellMeter/hooks", &serde_json::json!({}))
            .await
            .unwrap();
        server.join().unwrap();

        assert_eq!(response["id"], 1234);
    }

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
