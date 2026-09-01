use dlp::cache::MetadataCache;
use dlp::metadata::VideoMetadata;
use tempfile::tempdir;

#[test]
fn test_cache_miss_write_hit() {
    let temp = tempdir().unwrap();
    let cache = MetadataCache::with_dir(temp.path().to_path_buf(), 7200);

    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

    // 1. Initial cache miss
    assert!(cache.get(url).is_none());

    // 2. Set metadata in cache
    let meta = VideoMetadata {
        id: "dQw4w9WgXcQ".to_string(),
        title: "Rick Astley - Never Gonna Give You Up".to_string(),
        uploader: Some("RickAstleyVEVO".to_string()),
        duration: Some(213.0),
        width: Some(1920),
        height: Some(1080),
        ..Default::default()
    };

    cache.set(url, &meta, None);

    // 3. Subsequent cache hit
    let cached = cache.get(url).expect("expected cache hit");
    assert_eq!(cached.id, "dQw4w9WgXcQ");
    assert_eq!(cached.title, "Rick Astley - Never Gonna Give You Up");
    assert_eq!(cached.uploader.as_deref(), Some("RickAstleyVEVO"));
    assert_eq!(cached.duration, Some(213.0));
    assert_eq!(cached.height, Some(1080));
}

#[test]
fn test_expired_item_purge() {
    let temp = tempdir().unwrap();
    let cache = MetadataCache::with_dir(temp.path().to_path_buf(), 7200);

    let expired_url = "https://www.youtube.com/watch?v=expired123";
    let valid_url = "https://www.youtube.com/watch?v=valid456";

    let meta_expired = VideoMetadata {
        id: "expired123".to_string(),
        title: "Expired Video".to_string(),
        ..Default::default()
    };

    let meta_valid = VideoMetadata {
        id: "valid456".to_string(),
        title: "Valid Video".to_string(),
        ..Default::default()
    };

    // Write with TTL = 0 to expire immediately
    cache.set(expired_url, &meta_expired, Some(0));
    // Write with TTL = 3600 to remain valid
    cache.set(valid_url, &meta_valid, Some(3600));

    // purge_expired should identify and remove the expired item
    let purged_count = cache.purge_expired();
    assert_eq!(purged_count, 1);

    // Expired item should not be found
    assert!(cache.get(expired_url).is_none());

    // Valid item must still exist
    assert!(cache.get(valid_url).is_some());

    // Also test: setting another expired item and calling get() cleans it up on the fly
    let expired_url2 = "https://www.youtube.com/watch?v=expired789";
    cache.set(expired_url2, &meta_expired, Some(0));
    assert!(cache.cache_file_path(expired_url2).exists());
    assert!(cache.get(expired_url2).is_none());
    assert!(!cache.cache_file_path(expired_url2).exists());
}

#[test]
fn test_purge_all() {
    let temp = tempdir().unwrap();
    let cache = MetadataCache::with_dir(temp.path().to_path_buf(), 7200);

    let url1 = "https://www.youtube.com/watch?v=item1";
    let url2 = "https://www.youtube.com/watch?v=item2";
    let url3 = "https://www.youtube.com/watch?v=item3";

    let meta = VideoMetadata {
        id: "item".to_string(),
        title: "Item".to_string(),
        ..Default::default()
    };

    cache.set(url1, &meta, None);
    cache.set(url2, &meta, None);
    cache.set(url3, &meta, None);

    assert!(cache.get(url1).is_some());
    assert!(cache.get(url2).is_some());
    assert!(cache.get(url3).is_some());

    cache.purge_all().expect("purge_all should succeed");

    assert!(cache.get(url1).is_none());
    assert!(cache.get(url2).is_none());
    assert!(cache.get(url3).is_none());
}

#[test]
fn test_url_normalization_tracking_parameters() {
    let url_clean = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let url_si = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&si=abc123xyz";
    let url_feature = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&feature=share";
    let url_multi = "https://www.youtube.com/watch?feature=share&v=dQw4w9WgXcQ&si=xyz&utm_source=twitter";

    let key_clean = MetadataCache::normalize_key(url_clean);
    let key_si = MetadataCache::normalize_key(url_si);
    let key_feature = MetadataCache::normalize_key(url_feature);
    let key_multi = MetadataCache::normalize_key(url_multi);

    assert_eq!(key_clean, key_si);
    assert_eq!(key_clean, key_feature);
    assert_eq!(key_clean, key_multi);

    // Test short URLs with tracking params
    let short_clean = "https://youtu.be/dQw4w9WgXcQ";
    let short_si = "https://youtu.be/dQw4w9WgXcQ?si=tracking123";
    let short_feature = "https://youtu.be/dQw4w9WgXcQ?feature=shared";

    assert_eq!(
        MetadataCache::normalize_key(short_clean),
        MetadataCache::normalize_key(short_si)
    );
    assert_eq!(
        MetadataCache::normalize_key(short_clean),
        MetadataCache::normalize_key(short_feature)
    );

    // Test cache hit across normalized URLs
    let temp = tempdir().unwrap();
    let cache = MetadataCache::with_dir(temp.path().to_path_buf(), 7200);

    let meta = VideoMetadata {
        id: "dQw4w9WgXcQ".to_string(),
        title: "Normalized Title".to_string(),
        ..Default::default()
    };

    cache.set(url_si, &meta, None);

    // Querying with clean or feature-tagged URL hits the same cache item
    let hit_clean = cache.get(url_clean);
    assert!(hit_clean.is_some());
    assert_eq!(hit_clean.unwrap().title, "Normalized Title");

    let hit_feature = cache.get(url_feature);
    assert!(hit_feature.is_some());
}

#[test]
fn test_live_stream_not_cached() {
    let temp = tempdir().unwrap();
    let cache = MetadataCache::with_dir(temp.path().to_path_buf(), 7200);

    let live_url = "https://www.youtube.com/watch?v=live_stream_now";
    let live_meta = VideoMetadata {
        id: "live_stream_now".to_string(),
        title: "Live 24/7 News Stream".to_string(),
        is_live: Some(true),
        ..Default::default()
    };

    cache.set(live_url, &live_meta, None);

    // Cache get should return None
    assert!(cache.get(live_url).is_none());

    // File should not even have been created on disk
    let file_path = cache.cache_file_path(live_url);
    assert!(!file_path.exists());
}

#[test]
fn test_cache_dir_accessors() {
    let temp = tempdir().unwrap();
    let custom_dir = temp.path().join("subcache");
    let cache = MetadataCache::with_dir(custom_dir.clone(), 3600);

    assert_eq!(cache.cache_dir(), custom_dir.as_path());

    let default_cache = MetadataCache::new();
    assert!(default_cache.cache_dir().ends_with(std::path::Path::new("dlp").join("metadata")));
}
