use super::*;
use crate::commands::write_test_fixture::fixture;
use clap::Parser;

fn command(args: &[&str]) -> IssueCommands {
    let mut argv = vec!["github", "issue"];
    argv.extend_from_slice(args);
    match crate::Cli::try_parse_from(argv).unwrap().command {
        crate::Commands::Issue { command } => command,
        _ => unreachable!(),
    }
}

fn assert_request(request: &str, method: &str, path: &str, body: Value) {
    assert!(
        request.starts_with(&format!("{method} {path} HTTP/1.1\r\n")),
        "{request}"
    );
    let actual: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(actual, body);
}

#[tokio::test]
async fn create_posts_supplied_fields_and_preserves_multiline_body() {
    let (client, server) = fixture(vec![(
        "201 Created",
        r#"{"number":42,"title":"Bug","state":"open","html_url":"https://github.com/owner/repo/issues/42"}"#,
    )]);
    handle(
        &client,
        command(&[
            "create",
            "owner/repo",
            "-t",
            "Bug",
            "-b",
            "first\nsecond ✓",
            "--labels",
            "bug,ui",
            "--assignees",
            "alice,bob",
        ]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/issues",
        json!({"title":"Bug","body":"first\nsecond ✓","labels":["bug","ui"],"assignees":["alice","bob"]}),
    );
}

#[tokio::test]
async fn create_omits_unsupplied_fields_and_requires_title() {
    let (client, server) = fixture(vec![(
        "201 Created",
        r#"{"number":42,"title":"Bug","state":"open"}"#,
    )]);
    handle(
        &client,
        command(&["create", "owner/repo", "--title", "Bug"]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/issues",
        json!({"title":"Bug"}),
    );
    assert!(crate::Cli::try_parse_from(["github", "issue", "create", "owner/repo"]).is_err());
    assert!(
        crate::Cli::try_parse_from(["github", "issue", "comment", "owner/repo", "42"]).is_err()
    );
}

#[tokio::test]
async fn edit_changes_only_supplied_fields_and_can_clear_sets_and_body() {
    for (args, expected) in [
        (vec!["--title", "New"], json!({"title":"New"})),
        (
            vec!["--body", "", "--labels", "", "--assignees", ""],
            json!({"body":"","labels":[],"assignees":[]}),
        ),
        (
            vec!["--labels", "bug,urgent", "--assignees", "alice"],
            json!({"labels":["bug","urgent"],"assignees":["alice"]}),
        ),
    ] {
        let (client, server) = fixture(vec![(
            "200 OK",
            r#"{"number":42,"title":"New","state":"open"}"#,
        )]);
        let mut argv = vec!["edit", "owner/repo", "42"];
        argv.extend(args);
        handle(&client, command(&argv)).await.unwrap();
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_request(
            &requests[0],
            "PATCH",
            "/repos/owner/repo/issues/42",
            expected,
        );
    }
}

#[tokio::test]
async fn edit_rejects_no_changes_before_network_io() {
    let client = Client::for_test("test", "http://127.0.0.1:1").unwrap();
    let error = handle(&client, command(&["edit", "owner/repo", "42"]))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("No issue changes supplied"));
}

#[tokio::test]
async fn close_and_reopen_patch_requested_state() {
    for (action, state) in [("close", "closed"), ("reopen", "open")] {
        let (client, server) = fixture(vec![(
            "200 OK",
            r#"{"number":42,"title":"Bug","state":"open"}"#,
        )]);
        handle(&client, command(&[action, "owner/repo", "42"]))
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert_request(
            &requests[0],
            "PATCH",
            "/repos/owner/repo/issues/42",
            json!({"state":state}),
        );
    }
}

#[tokio::test]
async fn comment_posts_message_as_body() {
    let (client, server) = fixture(vec![("201 Created", r#"{"id":99}"#)]);
    handle(
        &client,
        command(&["comment", "owner/repo", "42", "-m", "Fixed\nDetails"]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/issues/42/comments",
        json!({"body":"Fixed\nDetails"}),
    );
}

#[tokio::test]
async fn writes_report_api_failure_without_retrying() {
    for args in [
        vec!["create", "owner/repo", "-t", "Bug"],
        vec!["edit", "owner/repo", "42", "-b", "New"],
        vec!["close", "owner/repo", "42"],
        vec!["reopen", "owner/repo", "42"],
        vec!["comment", "owner/repo", "42", "-m", "Note"],
    ] {
        let (client, server) = fixture(vec![(
            "422 Unprocessable Entity",
            r#"{"message":"Validation failed"}"#,
        )]);
        let error = handle(&client, command(&args)).await.unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("422"));
        assert!(message.contains("Validation failed"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
}
