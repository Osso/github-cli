use anyhow::{Result, bail};

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
        let resp = self.http.request(method.clone(), &url).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("{} {path} failed ({status}): {body}", method);
        }
        Ok(resp)
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        Ok(self.send(reqwest::Method::GET, path).await?.json().await?)
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        Ok(self.send_json(reqwest::Method::POST, path, body).await?.json().await?)
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
        Ok(self.send(reqwest::Method::GET, path).await?.bytes().await?)
    }

    pub async fn search_code(
        &self,
        query: &str,
        limit: u32,
        page: u32,
    ) -> Result<serde_json::Value> {
        let q = urlencoding::encode(query);
        let url = format!("https://api.github.com/search/code?q={q}&per_page={limit}&page={page}");
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
    }
}
