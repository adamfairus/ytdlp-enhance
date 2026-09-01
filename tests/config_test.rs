use dlp::config::{Config, DownloadPolicy};

#[test]
fn test_default_download_policy() {
    let policy = DownloadPolicy::default();
    assert_eq!(policy.concurrency, 1);
    assert_eq!(policy.retry_delay_sec, 2);
    assert_eq!(policy.rate_limit, None);

    let config = Config::default();
    assert_eq!(config.download.concurrency, 1);
    assert_eq!(config.download.retry_delay_sec, 2);
}

#[test]
fn test_parse_config_with_download_policy() {
    let toml_str = r#"
        default_preset = "music"
        download_dir = "/tmp/downloads"

        [download]
        concurrency = 4
        retry_delay_sec = 5
        rate_limit = "10M"
    "#;

    let parsed: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.default_preset, "music");
    assert_eq!(parsed.download_dir, Some("/tmp/downloads".to_string()));
    assert_eq!(parsed.download.concurrency, 4);
    assert_eq!(parsed.download.retry_delay_sec, 5);
    assert_eq!(parsed.download.rate_limit, Some("10M".to_string()));
}
