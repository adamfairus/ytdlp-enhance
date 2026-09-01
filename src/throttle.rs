use std::sync::Mutex;
use std::time::{Duration, Instant};
use crate::scheduler::PlatformCategory;

#[derive(Debug, Clone)]
pub struct TokenBucket {
    pub capacity: f64,
    pub tokens: f64,
    pub fill_rate_per_sec: f64,
    pub last_update: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f64, fill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            fill_rate_per_sec,
            last_update: Instant::now(),
        }
    }

    pub fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.fill_rate_per_sec).min(self.capacity);
        self.last_update = now;
    }

    pub fn try_acquire(&mut self, count: f64) -> bool {
        self.refill();
        if self.tokens >= count {
            self.tokens -= count;
            true
        } else {
            false
        }
    }

    /// Calculates duration needed to wait until `count` tokens are available.
    pub fn wait_duration(&mut self, count: f64) -> Duration {
        self.refill();
        if self.tokens >= count {
            Duration::ZERO
        } else {
            let needed = count - self.tokens;
            let secs = needed / self.fill_rate_per_sec;
            Duration::from_secs_f64(secs)
        }
    }
}

#[derive(Debug)]
pub struct PlatformRateLimiter {
    tiktok: Mutex<TokenBucket>,
    youtube: Mutex<TokenBucket>,
    generic: Mutex<TokenBucket>,
}

impl Default for PlatformRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformRateLimiter {
    pub fn new() -> Self {
        Self {
            // TikTok: max burst 2, refill 0.9/sec (min ~1.1s spacing per request)
            tiktok: Mutex::new(TokenBucket::new(2.0, 0.9)),
            // YouTube: max burst 8, refill 4.0/sec
            youtube: Mutex::new(TokenBucket::new(8.0, 4.0)),
            // Generic: max burst 10, refill 5.0/sec
            generic: Mutex::new(TokenBucket::new(10.0, 5.0)),
        }
    }

    pub fn acquire_permit(&self, platform: PlatformCategory) {
        let bucket_mutex = match platform {
            PlatformCategory::TikTok => &self.tiktok,
            PlatformCategory::YouTube | PlatformCategory::YouTubeMusic => &self.youtube,
            PlatformCategory::Generic => &self.generic,
        };

        let wait = {
            let mut bucket = bucket_mutex.lock().unwrap();
            let dur = bucket.wait_duration(1.0);
            if dur.is_zero() {
                bucket.try_acquire(1.0);
                Duration::ZERO
            } else {
                dur
            }
        };

        if !wait.is_zero() {
            std::thread::sleep(wait);
            let mut bucket = bucket_mutex.lock().unwrap();
            bucket.try_acquire(1.0);
        }
    }

    pub fn get_tokens(&self, platform: PlatformCategory) -> f64 {
        let bucket_mutex = match platform {
            PlatformCategory::TikTok => &self.tiktok,
            PlatformCategory::YouTube | PlatformCategory::YouTubeMusic => &self.youtube,
            PlatformCategory::Generic => &self.generic,
        };
        let mut bucket = bucket_mutex.lock().unwrap();
        bucket.refill();
        bucket.tokens
    }
}
