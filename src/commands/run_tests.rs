use super::*;
use serde_json::json;

#[test]
fn format_duration_secs_uses_compact_units() {
    assert_eq!(format_duration_secs(-1), "-");
    assert_eq!(format_duration_secs(0), "0s");
    assert_eq!(format_duration_secs(65), "1m05s");
    assert_eq!(format_duration_secs(3661), "1h01m01s");
}

#[test]
fn duration_between_handles_rfc3339_and_invalid_values() {
    assert_eq!(
        duration_between("2026-06-24T10:00:00Z", "2026-06-24T11:02:03Z"),
        "1h02m03s"
    );
    assert_eq!(duration_between("not-a-date", "2026-06-24T11:02:03Z"), "-");
}

#[test]
fn workflow_filter_matches_name_or_path() {
    let run = json!({
        "name": "Rust CI",
        "path": ".github/workflows/rust.yml"
    });

    assert!(run_matches_workflow_filter(&run, None));
    assert!(run_matches_workflow_filter(&run, Some("Rust")));
    assert!(run_matches_workflow_filter(&run, Some("rust.yml")));
    assert!(!run_matches_workflow_filter(&run, Some("deploy")));
}

#[test]
fn truncate_with_ellipsis_preserves_short_values_and_prefix() {
    assert_eq!(truncate_with_ellipsis("short", 10, 4), "short");
    assert_eq!(
        truncate_with_ellipsis("abcdefghijklmnopqrstuvwxyz", 10, 6),
        "abcdef…"
    );
}

#[test]
fn run_duration_uses_run_started_at_and_updated_at() {
    let run = json!({
        "run_started_at": "2026-06-24T10:00:00Z",
        "updated_at": "2026-06-24T10:00:42Z"
    });
    let missing_start = json!({
        "created_at": "2026-06-24T10:00:00Z",
        "updated_at": "2026-06-24T10:00:42Z"
    });

    assert_eq!(run_duration(&run), "42s");
    assert_eq!(run_duration(&missing_start), "-");
}

#[test]
fn string_array_keeps_only_strings() {
    assert_eq!(
        string_array(&json!(["ubuntu-latest", 42, "self-hosted"])),
        vec!["ubuntu-latest", "self-hosted"]
    );
    assert!(string_array(&json!({"labels": []})).is_empty());
}

#[test]
fn job_helpers_count_select_and_name_active_jobs() {
    let jobs = json!([
        {"id": 1, "name": "build", "status": "completed", "conclusion": "success"},
        {"id": 2, "name": "test", "status": "in_progress", "conclusion": null,
         "steps": [{"name": "checkout", "status": "completed"}, {"name": "cargo test", "status": "in_progress"}]},
        {"id": 3, "name": "lint", "status": "queued", "conclusion": null},
        {"id": 4, "name": "deploy", "status": "completed", "conclusion": "failure"},
        {"id": 5, "name": "cleanup", "status": "completed", "conclusion": "cancelled"}
    ]);
    let jobs_array = jobs.as_array().unwrap();
    let jobs_value = json!({ "jobs": jobs_array });

    assert_eq!(count_jobs(&jobs_value), (3, 1, 1, 2));
    assert_eq!(active_step_name(&jobs_array[1]), "cargo test");
    assert!(is_failed_job(&jobs_array[3]));
    assert!(is_failed_job(&jobs_array[4]));

    let failed = select_target_jobs(jobs_array, None);
    assert_eq!(failed.len(), 2);
    let named = select_target_jobs(jobs_array, Some("test"));
    assert_eq!(named, vec![&jobs_array[1]]);
}

#[test]
fn run_status_signature_includes_run_and_jobs() {
    let run = json!({
        "status": "in_progress",
        "conclusion": null,
        "updated_at": "2026-06-24T10:00:00Z"
    });
    let jobs = json!({
        "jobs": [
            {"id": 2, "name": "test", "status": "in_progress", "conclusion": null},
            {"id": 1, "name": "build", "status": "completed", "conclusion": "success"}
        ]
    });

    assert_eq!(
        run_status_signature(&run, &jobs),
        "in_progress|-|2026-06-24T10:00:00Z|2:in_progress:-:-|1:completed:success:-"
    );
}

#[test]
fn terminal_and_success_checks_use_status_and_conclusion() {
    assert!(is_terminal_run(&json!({"status": "completed"})));
    assert!(!is_terminal_run(&json!({"status": "in_progress"})));
    assert!(ensure_successful_run(&json!({"conclusion": "success"})).is_ok());
    assert!(ensure_successful_run(&json!({"conclusion": "failure"})).is_err());
}

#[test]
fn ensure_target_jobs_exist_reports_missing_filter() {
    let empty: Vec<&serde_json::Value> = Vec::new();

    assert!(ensure_target_jobs_exist(&empty, 42, None).is_ok());
    assert!(ensure_target_jobs_exist(&empty, 42, Some("missing")).is_err());
}
