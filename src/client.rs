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

    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("GET {path} failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    pub async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.post(&url).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("POST {path} failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    /// POST that expects no response body (e.g. 202 Cancel, 201 Rerun).
    pub async fn post_empty(&self, path: &str) -> Result<()> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.post(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("POST {path} failed ({status}): {body}");
        }
        Ok(())
    }

    pub async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.put(&url).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("PUT {path} failed ({status}): {body}");
        }
        let text = resp.text().await?;
        if text.is_empty() {
            Ok(serde_json::json!({}))
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.delete(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("DELETE {path} failed ({status}): {body}");
        }
        Ok(())
    }

    /// GET that follows redirects and returns the raw bytes (for log downloads).
    pub async fn get_bytes(&self, path: &str) -> Result<bytes::Bytes> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("GET {path} failed ({status}): {body}");
        }
        Ok(resp.bytes().await?)
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
