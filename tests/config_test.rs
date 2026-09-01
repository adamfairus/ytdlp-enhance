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

#[test]
fn test_config_mutation_and_validation() {
    let mut config = Config::default();

    config.set_value("default_preset", "music").unwrap();
    assert_eq!(config.default_preset, "music");

    config.set_value("download_dir", "/custom/path").unwrap();
    assert_eq!(config.download_dir, Some("/custom/path".to_string()));

    config.set_value("download_dir", "none").unwrap();
    assert_eq!(config.download_dir, None);

    config.set_value("concurrency", "4").unwrap();
    assert_eq!(config.download.concurrency, 4);

    config.set_value("retry_delay_sec", "10").unwrap();
    assert_eq!(config.download.retry_delay_sec, 10);

    config.set_value("rate_limit", "50M").unwrap();
    assert_eq!(config.download.rate_limit, Some("50M".to_string()));

    assert!(config.set_value("concurrency", "not_a_number").is_err());
    assert!(config.set_value("nonexistent_key", "value").is_err());
}

