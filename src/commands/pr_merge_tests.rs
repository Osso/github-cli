use super::*;
use clap::Parser;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn fixture(
    responses: Vec<(&'static str, &'static str)>,
) -> (Client, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        responses
            .into_iter()
            .map(|(status, body)| {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..end]);
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if data.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
                String::from_utf8(data).unwrap()
            })
            .collect()
    });
    (
        Client::for_test("test", &format!("http://{address}")).unwrap(),
        server,
    )
}

fn command(args: &[&str]) -> PrCommands {
    let mut argv = vec!["github", "pr", "merge", "owner/repo", "42"];
    argv.extend_from_slice(args);
    let cli = crate::Cli::try_parse_from(argv).unwrap();
    match cli.command {
        crate::Commands::Pr { command } => command,
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn merge_options_send_expected_payload() {
    for (args, method, sha) in [
        (vec![], "merge", None),
        (vec!["--merge"], "merge", None),
        (vec!["--squash", "--sha", "abc"], "squash", Some("abc")),
        (vec!["--rebase"], "rebase", None),
    ] {
        let (client, server) = fixture(vec![("200 OK", r#"{"merged":true,"sha":"result-sha"}"#)]);
        handle(&client, command(&args)).await.unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("PUT /repos/owner/repo/pulls/42/merge HTTP/1.1"));
        let body: serde_json::Value =
            serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["merge_method"], method);
        assert_eq!(body.get("sha").and_then(|v| v.as_str()), sha);
    }
    assert!(
        crate::Cli::try_parse_from([
            "github",
            "pr",
            "merge",
            "owner/repo",
            "42",
            "--squash",
            "--rebase"
        ])
        .is_err()
    );
}

#[tokio::test]
async fn merge_failure_reports_github_message_without_deleting() {
    let (client, server) = fixture(vec![(
        "409 Conflict",
        r#"{"message":"Head SHA was modified"}"#,
    )]);
    let error = handle(&client, command(&["--delete-branch"]))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Head SHA was modified"));
    assert_eq!(server.join().unwrap().len(), 1);

    let (client, server) = fixture(vec![(
        "200 OK",
        r#"{"merged":false,"message":"Not mergeable"}"#,
    )]);
    let error = handle(&client, command(&["--delete-branch"]))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Not mergeable"));
    assert_eq!(server.join().unwrap().len(), 1);
}

#[tokio::test]
async fn merge_deletes_only_same_repo_head_and_reports_failure() {
    let merged = ("200 OK", r#"{"merged":true,"sha":"merge-sha"}"#);
    let same_repo = (
        "200 OK",
        r#"{"head":{"repo":{"full_name":"owner/repo"},"ref":"feature/test"}}"#,
    );
    let (client, server) = fixture(vec![merged, same_repo, ("204 No Content", "")]);
    handle(&client, command(&["--delete-branch"]))
        .await
        .unwrap();
    let requests = server.join().unwrap();
    assert!(requests[1].starts_with("GET /repos/owner/repo/pulls/42 HTTP/1.1"));
    assert!(
        requests[2].starts_with("DELETE /repos/owner/repo/git/refs/heads/feature%2Ftest HTTP/1.1")
    );

    let fork = (
        "200 OK",
        r#"{"head":{"repo":{"full_name":"other/repo"},"ref":"feature"}}"#,
    );
    let (client, server) = fixture(vec![merged, fork]);
    handle(&client, command(&["--delete-branch"]))
        .await
        .unwrap();
    assert_eq!(server.join().unwrap().len(), 2);

    let (client, server) = fixture(vec![
        merged,
        same_repo,
        ("403 Forbidden", r#"{"message":"Protected branch"}"#),
    ]);
    let error = handle(&client, command(&["--delete-branch"]))
        .await
        .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("Merged, but failed to delete branch feature/test"));
    assert!(message.contains("Protected branch"));
    assert_eq!(server.join().unwrap().len(), 3);
}
