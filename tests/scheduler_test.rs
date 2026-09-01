use dlp::scheduler::{PlatformCategory, ScheduledPlan, TaskPriority, TaskScheduler, TaskState};

#[test]
fn test_platform_detection() {
    assert_eq!(
        ScheduledPlan::detect_platform("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
        PlatformCategory::YouTube
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://youtu.be/dQw4w9WgXcQ"),
        PlatformCategory::YouTube
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://music.youtube.com/watch?v=abcdef"),
        PlatformCategory::YouTubeMusic
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://www.tiktok.com/@creator/video/1234567890"),
        PlatformCategory::TikTok
    );
    assert_eq!(
        ScheduledPlan::detect_platform("https://example.com/stream.mp4"),
        PlatformCategory::Generic
    );
}

#[test]
fn test_scheduled_plan_grouping() {
    let urls = vec![
        "https://www.youtube.com/watch?v=vid1".to_string(),
        "https://music.youtube.com/watch?v=music1".to_string(),
        "https://www.tiktok.com/@creator/video/111".to_string(),
        "https://www.youtube.com/watch?v=vid2".to_string(),
        "https://www.tiktok.com/@creator/video/222".to_string(),
    ];

    let plan = ScheduledPlan::from_urls(&urls);
    assert_eq!(plan.tasks.len(), 5);

    let yt_tasks = plan.grouped.get(&PlatformCategory::YouTube).unwrap();
    assert_eq!(yt_tasks.len(), 2);

    let music_tasks = plan.grouped.get(&PlatformCategory::YouTubeMusic).unwrap();
    assert_eq!(music_tasks.len(), 1);

    let tiktok_tasks = plan.grouped.get(&PlatformCategory::TikTok).unwrap();
    assert_eq!(tiktok_tasks.len(), 2);
}

#[test]
fn test_platform_concurrency_capping() {
    // TikTok should cap to 2 to prevent rate-limit bans
    assert_eq!(PlatformCategory::TikTok.max_safe_concurrency(5), 2);
    assert_eq!(PlatformCategory::TikTok.max_safe_concurrency(1), 1);

    // YouTube and Generic should allow desired concurrency
    assert_eq!(PlatformCategory::YouTube.max_safe_concurrency(5), 5);
    assert_eq!(PlatformCategory::Generic.max_safe_concurrency(8), 8);
}

#[test]
fn test_priority_ordering() {
    let mut scheduler = TaskScheduler::new(4);
    let id_low = scheduler.add_task("https://www.youtube.com/watch?v=low", TaskPriority::Low);
    let id_normal = scheduler.add_task("https://www.youtube.com/watch?v=normal", TaskPriority::Normal);
    let id_urgent = scheduler.add_task("https://www.youtube.com/watch?v=urgent", TaskPriority::Urgent);
    let id_high = scheduler.add_task("https://www.youtube.com/watch?v=high", TaskPriority::High);

    // Urgent should be picked first
    let task1 = scheduler.next_runnable().expect("urgent task should be runnable");
    assert_eq!(task1.id, id_urgent);
    assert_eq!(task1.priority, TaskPriority::Urgent);
    assert_eq!(task1.state, TaskState::Running);

    // Next should be High
    let task2 = scheduler.next_runnable().expect("high task should be runnable");
    assert_eq!(task2.id, id_high);
    assert_eq!(task2.priority, TaskPriority::High);
    assert_eq!(task2.state, TaskState::Running);

    // Next should be Normal
    let task3 = scheduler.next_runnable().expect("normal task should be runnable");
    assert_eq!(task3.id, id_normal);
    assert_eq!(task3.priority, TaskPriority::Normal);
    assert_eq!(task3.state, TaskState::Running);

    // Next should be Low
    let task4 = scheduler.next_runnable().expect("low task should be runnable");
    assert_eq!(task4.id, id_low);
    assert_eq!(task4.priority, TaskPriority::Low);
    assert_eq!(task4.state, TaskState::Running);

    // None left runnable
    assert!(scheduler.next_runnable().is_none());
    assert_eq!(scheduler.running_count(), 4);
}

#[test]
fn test_platform_concurrency_bounding() {
    // Global concurrency is 8, but TikTok max safe concurrency is 2
    let mut scheduler = TaskScheduler::new(8);
    let tt1 = scheduler.add_task("https://www.tiktok.com/@creator/video/1", TaskPriority::Normal);
    let tt2 = scheduler.add_task("https://www.tiktok.com/@creator/video/2", TaskPriority::Normal);
    let tt3 = scheduler.add_task("https://www.tiktok.com/@creator/video/3", TaskPriority::Normal);
    let yt1 = scheduler.add_task("https://www.youtube.com/watch?v=yt1", TaskPriority::Normal);

    // First TikTok task runs (running TikTok: 1 <= 2)
    let r1 = scheduler.next_runnable().expect("tt1 should be runnable");
    assert_eq!(r1.id, tt1);
    assert_eq!(r1.platform, PlatformCategory::TikTok);

    // Second TikTok task runs (running TikTok: 2 <= 2)
    let r2 = scheduler.next_runnable().expect("tt2 should be runnable");
    assert_eq!(r2.id, tt2);
    assert_eq!(r2.platform, PlatformCategory::TikTok);

    // Third task: tt3 is blocked because TikTok has 2 running.
    // However, YouTube task yt1 has capacity and should be picked!
    let r3 = scheduler.next_runnable().expect("yt1 should be runnable");
    assert_eq!(r3.id, yt1);
    assert_eq!(r3.platform, PlatformCategory::YouTube);

    // Now tt3 cannot run because TikTok is at capacity (2), and no other tasks exist
    assert!(scheduler.next_runnable().is_none());
    assert_eq!(scheduler.running_count(), 3);

    // Complete one TikTok task
    scheduler.complete_task(tt1);
    assert_eq!(scheduler.running_count(), 2);
    assert_eq!(scheduler.completed_count(), 1);

    // Now tt3 becomes runnable because TikTok running drops to 1
    let r4 = scheduler.next_runnable().expect("tt3 should be runnable after slot frees");
    assert_eq!(r4.id, tt3);
    assert_eq!(r4.platform, PlatformCategory::TikTok);

    // No further runnable tasks
    assert!(scheduler.next_runnable().is_none());
}

#[test]
fn test_state_transitions() {
    let mut scheduler = TaskScheduler::new(2);
    let id = scheduler.add_task("https://www.youtube.com/watch?v=test", TaskPriority::Normal);

    // Initial state: Pending
    assert_eq!(scheduler.get_task(id).unwrap().state, TaskState::Pending);
    assert_eq!(scheduler.get_task(id).unwrap().retry_count, 0);

    // Pending -> Running
    let task = scheduler.next_runnable().unwrap();
    assert_eq!(task.id, id);
    assert_eq!(task.state, TaskState::Running);
    assert_eq!(scheduler.get_task(id).unwrap().state, TaskState::Running);
    assert_eq!(scheduler.running_count(), 1);

    // Running -> Retrying (attempt 1)
    scheduler.fail_task(id, true, 2);
    assert_eq!(scheduler.get_task(id).unwrap().state, TaskState::Retrying { attempt: 1 });
    assert_eq!(scheduler.get_task(id).unwrap().retry_count, 1);
    assert_eq!(scheduler.running_count(), 0);

    // Retrying -> Running
    let task = scheduler.next_runnable().unwrap();
    assert_eq!(task.id, id);
    assert_eq!(task.state, TaskState::Running);
    assert_eq!(scheduler.running_count(), 1);

    // Running -> Completed
    scheduler.complete_task(id);
    assert_eq!(scheduler.get_task(id).unwrap().state, TaskState::Completed);
    assert_eq!(scheduler.running_count(), 0);
    assert_eq!(scheduler.completed_count(), 1);

    // Non-retryable failure: Running -> Failed
    let id_fail = scheduler.add_task("https://www.youtube.com/watch?v=fail", TaskPriority::Normal);
    let _ = scheduler.next_runnable().unwrap();
    scheduler.fail_task(id_fail, false, 3);
    assert_eq!(scheduler.get_task(id_fail).unwrap().state, TaskState::Failed);
    assert_eq!(scheduler.failed_count(), 1);

    // Exhausted retries: Running -> Retrying -> Running -> Failed
    let id_exhaust = scheduler.add_task("https://www.youtube.com/watch?v=exhaust", TaskPriority::Normal);
    let _ = scheduler.next_runnable().unwrap();
    scheduler.fail_task(id_exhaust, true, 1); // 1st retry: attempt 1
    assert_eq!(scheduler.get_task(id_exhaust).unwrap().state, TaskState::Retrying { attempt: 1 });

    let _ = scheduler.next_runnable().unwrap(); // resumes running
    scheduler.fail_task(id_exhaust, true, 1); // retry_count (1) is no longer < max_retries (1) -> Failed
    assert_eq!(scheduler.get_task(id_exhaust).unwrap().state, TaskState::Failed);
    assert_eq!(scheduler.failed_count(), 2);
}

#[test]
fn test_scheduler_is_finished() {
    let urls = vec![
        "https://www.youtube.com/watch?v=1".to_string(),
        "https://www.youtube.com/watch?v=2".to_string(),
    ];
    let mut scheduler = TaskScheduler::from_urls(&urls, 2);
    assert!(!scheduler.is_finished());

    let t1 = scheduler.next_runnable().unwrap();
    assert!(!scheduler.is_finished());

    let t2 = scheduler.next_runnable().unwrap();
    assert!(!scheduler.is_finished());

    scheduler.complete_task(t1.id);
    assert!(!scheduler.is_finished());

    scheduler.fail_task(t2.id, false, 0);
    assert!(scheduler.is_finished());
    assert_eq!(scheduler.completed_count(), 1);
    assert_eq!(scheduler.failed_count(), 1);
    assert_eq!(scheduler.running_count(), 0);
}

#[test]
fn test_global_concurrency_limit() {
    let mut scheduler = TaskScheduler::new(2);
    scheduler.add_task("https://www.youtube.com/watch?v=1", TaskPriority::Normal);
    scheduler.add_task("https://www.youtube.com/watch?v=2", TaskPriority::Normal);
    scheduler.add_task("https://www.youtube.com/watch?v=3", TaskPriority::Normal);

    let t1 = scheduler.next_runnable().unwrap();
    let t2 = scheduler.next_runnable().unwrap();

    // Concurrency limit of 2 reached
    assert!(scheduler.next_runnable().is_none());
    assert_eq!(scheduler.running_count(), 2);

    scheduler.complete_task(t1.id);
    assert_eq!(scheduler.running_count(), 1);

    // Now 3rd task can run
    let t3 = scheduler.next_runnable().unwrap();
    assert_eq!(t3.id, 3);
    assert_eq!(scheduler.running_count(), 2);

    assert!(scheduler.next_runnable().is_none());

    scheduler.complete_task(t2.id);
    scheduler.complete_task(t3.id);
    assert!(scheduler.is_finished());
}
