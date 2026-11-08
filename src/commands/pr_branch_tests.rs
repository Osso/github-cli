use super::*;
use crate::commands::write_test_fixture::fixture;
use std::path::PathBuf;

/// A clone whose `upstream` remote points at the parent repository, as a fork checkout has.
struct ForkClone {
    dir: PathBuf,
}

impl ForkClone {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("github-pr-branch-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clone = Self { dir };
        for args in [
            &["init", "-q", "-b", "master"][..],
            &["remote", "add", "origin", "git@github.com:Osso/pi.git"],
            &[
                "remote",
                "add",
                "upstream",
                "git@github.com:earendil-works/pi.git",
            ],
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ],
            &["branch", "unpushed"],
            &["config", "branch.master.remote", "origin"],
            &["config", "branch.master.merge", "refs/heads/master"],
        ] {
            git(&clone.dir, args).unwrap();
        }
        clone
    }
}

impl Drop for ForkClone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn branch_pull_request_queries_the_push_remote_not_upstream() {
    let clone = ForkClone::new();
    let (client, server) = fixture(vec![
        (
            "200 OK",
            r#"[{"number":6,"html_url":"https://github.com/Osso/pi/pull/6","state":"closed","merged_at":"2026-11-01T00:00:00Z","head":{"sha":"abc123"}}]"#,
        ),
        (
            "200 OK",
            r#"{"check_runs":[{"status":"completed","conclusion":"success"},{"status":"completed","conclusion":"skipped"},{"status":"in_progress","conclusion":null},{"status":"completed","conclusion":"timed_out"}]}"#,
        ),
        (
            "200 OK",
            r#"{"statuses":[{"state":"pending"},{"state":"error"},{"state":"success"}]}"#,
        ),
    ]);

    let pull_request = branch_pull_request(&client, &clone.dir, "master")
        .await
        .unwrap();
    let requests = server.join().unwrap();

    assert!(
        requests[0].starts_with(
            "GET /repos/Osso/pi/pulls?head=Osso%3Amaster&state=all&per_page=1 HTTP/1.1"
        )
    );
    assert!(requests[1].starts_with("GET /repos/Osso/pi/commits/abc123/check-runs?per_page=100 "));
    assert!(requests[2].starts_with("GET /repos/Osso/pi/commits/abc123/status "));
    assert_eq!(
        pull_request,
        Some(BranchPullRequest {
            number: 6,
            url: "https://github.com/Osso/pi/pull/6".to_owned(),
            state: "merged".to_owned(),
            checks: Checks {
                passed: 3,
                running: 2,
                failed: 2
            },
        })
    );
    assert_eq!(
        serde_json::to_value(&pull_request).unwrap(),
        serde_json::json!({
            "number": 6,
            "url": "https://github.com/Osso/pi/pull/6",
            "state": "merged",
            "checks": {"passed": 3, "running": 2, "failed": 2}
        })
    );
}

#[tokio::test]
async fn branch_without_pull_request_or_push_remote_has_none() {
    let clone = ForkClone::new();
    let (client, server) = fixture(vec![("200 OK", "[]")]);
    assert_eq!(
        branch_pull_request(&client, &clone.dir, "master")
            .await
            .unwrap(),
        None
    );
    assert_eq!(server.join().unwrap().len(), 1);

    let (client, server) = fixture(vec![]);
    assert_eq!(
        branch_pull_request(&client, &clone.dir, "unpushed")
            .await
            .unwrap(),
        None
    );
    assert!(server.join().unwrap().is_empty());
}

#[test]
fn github_repository_accepts_github_remote_urls_only() {
    for url in [
        "git@github.com:Osso/pi.git",
        "https://github.com/Osso/pi.git",
        "https://github.com/Osso/pi",
        "https://token@github.com/Osso/pi/",
        "ssh://git@github.com/Osso/pi.git",
    ] {
        assert_eq!(github_repository(url).as_deref(), Some("Osso/pi"), "{url}");
    }
    for url in [
        "git@gitlab.com:Osso/pi.git",
        "git@github.com.evil.test:Osso/pi.git",
        "git@evilgithub.com:Osso/pi.git",
        "https://github.com/Osso",
        "https://github.com/Osso/pi/tree/master",
    ] {
        assert_eq!(github_repository(url), None, "{url}");
    }
}
