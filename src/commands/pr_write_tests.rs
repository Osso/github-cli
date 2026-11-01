use super::*;
use crate::commands::write_test_fixture::fixture;
use clap::Parser;
use serde_json::{Value, json};

fn command(args: &[&str]) -> PrCommands {
    let mut argv = vec!["github", "pr"];
    argv.extend_from_slice(args);
    match crate::Cli::try_parse_from(argv).unwrap().command {
        crate::Commands::Pr { command } => command,
        _ => unreachable!(),
    }
}

fn assert_request(request: &str, method: &str, path: &str, expected: Value) {
    assert!(
        request.starts_with(&format!("{method} {path} HTTP/1.1\r\n")),
        "{request}"
    );
    let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, expected);
}

#[tokio::test]
async fn create_and_metadata_failure_are_not_retried() {
    let (client, server) = fixture(vec![
        ("201 Created", r#"{"number":42}"#),
        ("403 Forbidden", r#"{"message":"denied"}"#),
    ]);
    let error = handle(
        &client,
        command(&[
            "create",
            "owner/repo",
            "-t",
            "Title",
            "-b",
            "Body",
            "--head",
            "feature",
            "--base",
            "main",
            "--draft",
            "--labels",
            "bug,urgent",
            "--assignees",
            "alice",
        ]),
    )
    .await
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("Created owner/repo#42"));
    assert!(message.contains("denied"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/pulls",
        json!({"title":"Title","body":"Body","head":"feature","base":"main","draft":true}),
    );
    assert_request(
        &requests[1],
        "PATCH",
        "/repos/owner/repo/issues/42",
        json!({"labels":["bug","urgent"],"assignees":["alice"]}),
    );
}

#[tokio::test]
async fn create_omits_optional_body_and_metadata() {
    let (client, server) = fixture(vec![("201 Created", r#"{"number":7}"#)]);
    handle(
        &client,
        command(&[
            "create",
            "owner/repo",
            "--title",
            "T",
            "--head",
            "h",
            "--base",
            "b",
        ]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/pulls",
        json!({"title":"T","head":"h","base":"b","draft":false}),
    );
    assert!(
        crate::Cli::try_parse_from([
            "github",
            "pr",
            "create",
            "owner/repo",
            "--head",
            "h",
            "--base",
            "b"
        ])
        .is_err()
    );
    assert!(
        crate::Cli::try_parse_from([
            "github",
            "pr",
            "create",
            "owner/repo",
            "-t",
            "T",
            "--head",
            "h"
        ])
        .is_err()
    );
}

#[tokio::test]
async fn edit_sends_only_supplied_fields_and_reports_partial_failure() {
    let (client, server) = fixture(vec![
        ("200 OK", "{}"),
        ("422 Unprocessable Entity", r#"{"message":"invalid label"}"#),
    ]);
    let error = handle(
        &client,
        command(&[
            "edit",
            "owner/repo",
            "42",
            "-t",
            "New",
            "-b",
            "text",
            "--base",
            "next",
            "--labels",
            "bug",
        ]),
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("Updated owner/repo#42, but failed to update metadata"));
    assert!(format!("{error:#}").contains("invalid label"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_request(
        &requests[0],
        "PATCH",
        "/repos/owner/repo/pulls/42",
        json!({"title":"New","body":"text","base":"next"}),
    );
    assert_request(
        &requests[1],
        "PATCH",
        "/repos/owner/repo/issues/42",
        json!({"labels":["bug"]}),
    );
}

#[tokio::test]
async fn metadata_only_clear_and_noop_edit() {
    let (client, server) = fixture(vec![("200 OK", "{}")]);
    handle(
        &client,
        command(&["edit", "owner/repo", "42", "--labels", ""]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_request(
        &requests[0],
        "PATCH",
        "/repos/owner/repo/issues/42",
        json!({"labels":[]}),
    );
    let (client, server) = fixture(vec![]);
    let error = handle(&client, command(&["edit", "owner/repo", "42"]))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("No changes specified"));
    assert!(server.join().unwrap().is_empty());
}

#[tokio::test]
async fn create_with_metadata_succeeds_and_edit_can_clear_body_and_assignees() {
    let (client, server) = fixture(vec![("201 Created", r#"{"number":42}"#), ("200 OK", "{}")]);
    handle(
        &client,
        command(&[
            "create",
            "owner/repo",
            "-t",
            "T",
            "--head",
            "alice:topic",
            "--base",
            "main",
            "--labels",
            "bug",
            "--assignees",
            "alice,bob",
        ]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_request(
        &requests[0],
        "POST",
        "/repos/owner/repo/pulls",
        json!({"title":"T","head":"alice:topic","base":"main","draft":false}),
    );
    assert_request(
        &requests[1],
        "PATCH",
        "/repos/owner/repo/issues/42",
        json!({"labels":["bug"],"assignees":["alice","bob"]}),
    );

    let (client, server) = fixture(vec![("200 OK", "{}"), ("200 OK", "{}")]);
    handle(
        &client,
        command(&["edit", "owner/repo", "42", "--body", "", "--assignees", ""]),
    )
    .await
    .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_request(
        &requests[0],
        "PATCH",
        "/repos/owner/repo/pulls/42",
        json!({"body":""}),
    );
    assert_request(
        &requests[1],
        "PATCH",
        "/repos/owner/repo/issues/42",
        json!({"assignees":[]}),
    );
}

#[tokio::test]
async fn initial_write_failure_is_reported_without_metadata_write() {
    for args in [
        vec![
            "create",
            "owner/repo",
            "-t",
            "T",
            "--head",
            "h",
            "--base",
            "b",
            "--labels",
            "bug",
        ],
        vec!["edit", "owner/repo", "42", "-t", "New", "--labels", "bug"],
    ] {
        let (client, server) = fixture(vec![(
            "422 Unprocessable Entity",
            r#"{"message":"Validation failed"}"#,
        )]);
        let error = handle(&client, command(&args)).await.unwrap_err();
        assert!(format!("{error:#}").contains("Validation failed"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn close_and_reopen_send_state_and_propagate_errors() {
    for (operation, state) in [("close", "closed"), ("reopen", "open")] {
        let (client, server) = fixture(vec![("200 OK", "{}")]);
        handle(&client, command(&[operation, "owner/repo", "42"]))
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert_request(
            &requests[0],
            "PATCH",
            "/repos/owner/repo/pulls/42",
            json!({"state":state}),
        );
    }
    let (client, server) = fixture(vec![("403 Forbidden", r#"{"message":"not allowed"}"#)]);
    let error = handle(&client, command(&["close", "owner/repo", "42"]))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("not allowed"));
    assert_eq!(server.join().unwrap().len(), 1);
}
