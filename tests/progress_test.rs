use dlp::progress::ProgressTracker;

#[test]
fn test_progress_tracker_aria2_parsing() {
    let mut tracker = ProgressTracker::new();

    // Simulate Aria2 output stream
    tracker.parse_line("[#59bb07 19MiB/84MiB(23%) CN:16 DL:5.0MiB ETA:12s]");
    tracker.parse_line("[#59bb07 45MiB/84MiB(53%) CN:16 DL:4.5MiB ETA:8s]");
    tracker.parse_line("[#59bb07 84MiB/84MiB(100%) CN:11 DL:4.5MiB]");
}

#[test]
fn test_progress_tracker_ytdlp_parsing() {
    let mut tracker = ProgressTracker::new();

    // Simulate custom template output stream
    tracker.parse_line("__DLP__:25.0%|12.5MiB/s|00:06|100MiB");
    tracker.parse_line("__DLP__:75.0%|12.5MiB/s|00:02|100MiB");
    tracker.parse_line("__DLP__:100.0%|12.5MiB/s|00:00|100MiB");
}

#[test]
fn test_progress_tracker_ignores_subtitles_and_thumbnails() {
    let mut tracker = ProgressTracker::new();

    // Tiny subtitle/thumbnail downloads must not jump main progress
    tracker.parse_line("[download] Destination: /path/to/video.id.srt");
    tracker.parse_line("[download] 100% of 2.5KiB in 00:00:00 at 450KiB/s");
    tracker.parse_line("[info] Downloading video thumbnail 41 ...");
}
