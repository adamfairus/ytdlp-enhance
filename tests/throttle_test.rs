use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use dlp::scheduler::PlatformCategory;
use dlp::throttle::{PlatformRateLimiter, TokenBucket};

#[test]
fn test_token_bucket_initial_capacity() {
    let bucket = TokenBucket::new(10.0, 2.0);
    assert_eq!(bucket.capacity, 10.0);
    assert_eq!(bucket.tokens, 10.0);
    assert_eq!(bucket.fill_rate_per_sec, 2.0);
}

#[test]
fn test_token_bucket_refill_and_capping() {
    let mut bucket = TokenBucket::new(10.0, 5.0);

    // Consume all tokens
    assert!(bucket.try_acquire(10.0));
    assert!(bucket.tokens < 0.001);

    // Cannot acquire more immediately
    assert!(!bucket.try_acquire(1.0));

    // Simulate passage of 0.4s (should add 0.4 * 5.0 = 2.0 tokens)
    bucket.last_update = Instant::now() - Duration::from_millis(400);
    bucket.refill();
    assert!(bucket.tokens >= 1.9 && bucket.tokens <= 2.1);

    // Simulate passage of 10s (should be capped at capacity 10.0)
    bucket.last_update = Instant::now() - Duration::from_secs(10);
    bucket.refill();
    assert_eq!(bucket.tokens, 10.0);
}

#[test]
fn test_token_bucket_try_acquire() {
    let mut bucket = TokenBucket::new(5.0, 1.0);

    // Acquire partial tokens
    assert!(bucket.try_acquire(2.0));
    assert!(bucket.tokens >= 2.9 && bucket.tokens <= 3.1);

    // Acquire remaining tokens
    assert!(bucket.try_acquire(3.0));
    assert!(bucket.tokens < 0.1);

    // Exceed available tokens
    assert!(!bucket.try_acquire(1.0));
}

#[test]
fn test_token_bucket_wait_duration() {
    let mut bucket = TokenBucket::new(2.0, 2.0);

    // When full, wait duration is ZERO
    assert_eq!(bucket.wait_duration(1.0), Duration::ZERO);
    assert_eq!(bucket.wait_duration(2.0), Duration::ZERO);

    // Acquire all 2 tokens
    assert!(bucket.try_acquire(2.0));

    // For 1 token at 2 tokens/sec, wait duration should be ~0.5s (500ms)
    let wait = bucket.wait_duration(1.0);
    let wait_ms = wait.as_millis();
    assert!(wait_ms >= 450 && wait_ms <= 550, "Expected ~500ms, got {}ms", wait_ms);

    // Simulate 250ms elapsed -> remaining wait should be ~250ms
    bucket.last_update = Instant::now() - Duration::from_millis(250);
    let wait2 = bucket.wait_duration(1.0);
    let wait2_ms = wait2.as_millis();
    assert!(wait2_ms >= 200 && wait2_ms <= 300, "Expected ~250ms, got {}ms", wait2_ms);

    // Simulate 600ms elapsed -> tokens refilled to > 1.0 -> wait duration should be ZERO
    bucket.last_update = Instant::now() - Duration::from_millis(600);
    let wait3 = bucket.wait_duration(1.0);
    assert_eq!(wait3, Duration::ZERO);
    assert!(bucket.try_acquire(1.0));
}

#[test]
fn test_platform_rate_limiter_defaults() {
    let limiter = PlatformRateLimiter::new();

    // Verify initial token counts for each platform
    assert_eq!(limiter.get_tokens(PlatformCategory::TikTok), 2.0);
    assert_eq!(limiter.get_tokens(PlatformCategory::YouTube), 8.0);
    assert_eq!(limiter.get_tokens(PlatformCategory::YouTubeMusic), 8.0);
    assert_eq!(limiter.get_tokens(PlatformCategory::Generic), 10.0);

    let default_limiter = PlatformRateLimiter::default();
    assert_eq!(default_limiter.get_tokens(PlatformCategory::TikTok), 2.0);
}

#[test]
fn test_platform_rate_limiter_acquire_permit() {
    let limiter = PlatformRateLimiter::new();

    // YouTube permit acquisition
    limiter.acquire_permit(PlatformCategory::YouTube);
    let yt_tokens = limiter.get_tokens(PlatformCategory::YouTube);
    assert!(yt_tokens >= 6.9 && yt_tokens <= 7.1);

    // YouTubeMusic shares YouTube bucket
    limiter.acquire_permit(PlatformCategory::YouTubeMusic);
    let ytm_tokens = limiter.get_tokens(PlatformCategory::YouTube);
    assert!(ytm_tokens >= 5.9 && ytm_tokens <= 6.1);

    // TikTok permit acquisition
    limiter.acquire_permit(PlatformCategory::TikTok);
    let tt_tokens = limiter.get_tokens(PlatformCategory::TikTok);
    assert!(tt_tokens >= 0.9 && tt_tokens <= 1.1);

    // Generic permit acquisition
    limiter.acquire_permit(PlatformCategory::Generic);
    let gen_tokens = limiter.get_tokens(PlatformCategory::Generic);
    assert!(gen_tokens >= 8.9 && gen_tokens <= 9.1);
}

#[test]
fn test_platform_rate_limiter_concurrent_no_deadlock() {
    let limiter = Arc::new(PlatformRateLimiter::new());
    let mut handles = Vec::new();

    // Spawn 8 worker threads doing concurrent permit acquisitions
    for i in 0..8 {
        let limiter = Arc::clone(&limiter);
        let handle = thread::spawn(move || {
            let platform = match i % 4 {
                0 => PlatformCategory::YouTube,
                1 => PlatformCategory::YouTubeMusic,
                2 => PlatformCategory::Generic,
                _ => PlatformCategory::TikTok,
            };

            // Acquire permits across simulated operations
            for _ in 0..3 {
                limiter.acquire_permit(platform);
            }
        });
        handles.push(handle);
    }

    // Ensure all threads complete cleanly without deadlocking
    for handle in handles {
        handle.join().expect("Thread should not panic or deadlock");
    }
}
