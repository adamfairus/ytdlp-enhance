use std::io::Write;
use std::path::Path;
use tempfile::{tempdir, NamedTempFile};
use dlp::batch::{
    determine_checkpoint_path, read_urls_from_file, resolve_inputs_to_urls, run_batch_parallel,
    BatchCheckpoint, BatchReport, ItemStatus,
};
use dlp::preset::PresetManager;

#[test]
fn test_read_urls_from_file_with_comments_and_whitespace() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "# This is a comment").unwrap();
    writeln!(file, "https://youtube.com/watch?v=11111111111").unwrap();
    writeln!(file, "   ").unwrap();
    writeln!(file, "// Another comment").unwrap();
    writeln!(file, "  https://youtube.com/watch?v=22222222222  ").unwrap();
    writeln!(file, "").unwrap();
    writeln!(file, "https://music.youtube.com/watch?v=33333333333").unwrap();

    let urls = read_urls_from_file(file.path()).unwrap();
    assert_eq!(urls.len(), 3);
    assert_eq!(urls[0], "https://youtube.com/watch?v=11111111111");
    assert_eq!(urls[1], "https://youtube.com/watch?v=22222222222");
    assert_eq!(urls[2], "https://music.youtube.com/watch?v=33333333333");
}

#[test]
fn test_resolve_inputs_combines_files_and_direct_urls() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "https://youtube.com/watch?v=file_url_1").unwrap();
    writeln!(file, "https://youtube.com/watch?v=file_url_2").unwrap();

    let inputs = vec![
        file.path().to_str().unwrap().to_string(),
        "https://youtube.com/watch?v=direct_url_3".to_string(),
    ];

    let resolved = resolve_inputs_to_urls(&inputs).unwrap();
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0], "https://youtube.com/watch?v=file_url_1");
    assert_eq!(resolved[1], "https://youtube.com/watch?v=file_url_2");
    assert_eq!(resolved[2], "https://youtube.com/watch?v=direct_url_3");
}

#[test]
fn test_batch_report_calculation() {
    let report = BatchReport {
        total: 4,
        succeeded: 2,
        skipped: 1,
        failed: vec![("https://example.com/bad".to_string(), "404 Not Found".to_string())],
    };

    assert_eq!(report.total, 4);
    assert_eq!(report.succeeded, 2);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.failed.len(), 1);
}

#[test]
fn test_batch_checkpoint_save_and_load() {
    let dir = tempdir().unwrap();
    let cp_file = dir.path().join("test_batch.dlp_checkpoint.json");

    let mut cp = BatchCheckpoint::new();
    cp.mark_completed("https://youtube.com/watch?v=item1", "Item 1 Video");
    cp.mark_failed("https://youtube.com/watch?v=item2", "Format 399 unavailable");

    cp.save_to_path(&cp_file).unwrap();
    assert!(cp_file.exists());

    let loaded = BatchCheckpoint::load_from_path(&cp_file);
    assert!(loaded.is_completed("https://youtube.com/watch?v=item1"));
    assert!(!loaded.is_completed("https://youtube.com/watch?v=item2"));
    assert!(!loaded.is_completed("https://youtube.com/watch?v=item3"));

    match loaded.get_status("https://youtube.com/watch?v=item1") {
        Some(ItemStatus::Completed { title, .. }) => {
            assert_eq!(title, "Item 1 Video");
        }
        _ => panic!("Expected Completed status for item1"),
    }

    match loaded.get_status("https://youtube.com/watch?v=item2") {
        Some(ItemStatus::Failed { error, .. }) => {
            assert!(error.contains("Format 399 unavailable"));
        }
        _ => panic!("Expected Failed status for item2"),
    }
}

#[test]
fn test_determine_checkpoint_path() {
    let file = NamedTempFile::new().unwrap();
    let file_path = file.path().to_str().unwrap().to_string();

    let inputs = vec![file_path.clone()];
    let cp = determine_checkpoint_path(&inputs, None);
    assert_eq!(cp, Path::new(&format!("{}.dlp_checkpoint.json", file_path)));

    let direct_inputs = vec!["https://youtube.com/watch?v=direct".to_string()];
    let cp2 = determine_checkpoint_path(&direct_inputs, Some("/tmp/custom_dl"));
    assert_eq!(cp2, Path::new("/tmp/custom_dl/.dlp_checkpoint.json"));
}

#[test]
fn test_run_batch_parallel_task_scheduler_integration() {
    let dir = tempdir().unwrap();
    let cp_path = dir.path().join("scheduler_batch.dlp_checkpoint.json");
    let preset_mgr = PresetManager::load_all();

    let urls = vec![
        "invalid_scheme://batch_sched_1".to_string(),
        "invalid_scheme://batch_sched_2".to_string(),
    ];

    let result = run_batch_parallel(
        &urls,
        &preset_mgr,
        None,
        None,
        None,
        Some(dir.path().to_str().unwrap()),
        false,
        &cp_path,
        2,
    );

    assert!(result.is_ok(), "run_batch_parallel with TaskScheduler should succeed");
    let report = result.unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(report.failed.len(), 2);
    assert_eq!(report.succeeded, 0);
    assert_eq!(report.skipped, 0);

    // Verify that checkpoint recorded the failed items
    assert!(cp_path.exists());
    let checkpoint = BatchCheckpoint::load_from_path(&cp_path);
    assert!(matches!(checkpoint.get_status(&urls[0]), Some(ItemStatus::Failed { .. })));
    assert!(matches!(checkpoint.get_status(&urls[1]), Some(ItemStatus::Failed { .. })));
}

#[test]
fn test_run_batch_parallel_resume_with_scheduler() {
    let dir = tempdir().unwrap();
    let cp_path = dir.path().join("resume_scheduler_batch.dlp_checkpoint.json");
    let preset_mgr = PresetManager::load_all();

    let mut cp = BatchCheckpoint::new();
    cp.mark_completed("invalid_scheme://already_done", "Completed Media");
    cp.save_to_path(&cp_path).unwrap();

    let urls = vec![
        "invalid_scheme://already_done".to_string(),
        "invalid_scheme://new_item".to_string(),
    ];

    let result = run_batch_parallel(
        &urls,
        &preset_mgr,
        None,
        None,
        None,
        Some(dir.path().to_str().unwrap()),
        true,
        &cp_path,
        2,
    );

    assert!(result.is_ok());
    let report = result.unwrap();
    assert_eq!(report.total, 2);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.succeeded, 0);
}

#[test]
fn test_read_urls_from_markdown_file() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("playlist.md");
    let markdown_content = r#"---
title: My Playlist
date: 2026-04-11
---

# 🎵 Playlist Title

- [ ] [Track 1](https://youtu.be/video1?si=123) `3840x2160` (03:10)
- [x] [Track 2](https://youtu.be/video2?si=456) `2160x3840` (04:25)
* [ ] https://youtu.be/video3
- Just a plain line https://youtu.be/video4 trailing text
// Comment line
# Header
"#;
    std::fs::write(&md_path, markdown_content).unwrap();

    let urls = read_urls_from_file(&md_path).unwrap();
    assert_eq!(urls.len(), 4);
    assert_eq!(urls[0], "https://youtu.be/video1?si=123");
    assert_eq!(urls[1], "https://youtu.be/video2?si=456");
    assert_eq!(urls[2], "https://youtu.be/video3");
    assert_eq!(urls[3], "https://youtu.be/video4");
}


