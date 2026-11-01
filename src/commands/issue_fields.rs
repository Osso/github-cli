use clap::Args;
use serde_json::{Value, json};

#[derive(Args, Default)]
pub struct MetadataArgs {
    /// Replace all labels (comma-separated); use --labels "" to clear
    #[arg(long, value_delimiter = ',')]
    pub labels: Option<Vec<String>>,
    /// Replace all assignees (comma-separated); use --assignees "" to clear
    #[arg(long, value_delimiter = ',')]
    pub assignees: Option<Vec<String>>,
}

impl MetadataArgs {
    pub fn append_to(&self, body: &mut Value) {
        if let Some(labels) = &self.labels {
            body["labels"] = json!(nonempty_values(labels));
        }
        if let Some(assignees) = &self.assignees {
            body["assignees"] = json!(nonempty_values(assignees));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.labels.is_none() && self.assignees.is_none()
    }
}

fn nonempty_values(values: &[String]) -> Vec<&str> {
    values
        .iter()
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .collect()
}
