use std::io::Write;
use tempfile::NamedTempFile;
use dlp::batch::{read_urls_from_file, resolve_inputs_to_urls, BatchReport};

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
        total: 3,
        succeeded: 2,
        failed: vec![("https://example.com/bad".to_string(), "404 Not Found".to_string())],
    };

    assert_eq!(report.total, 3);
    assert_eq!(report.succeeded, 2);
    assert_eq!(report.failed.len(), 1);
}
