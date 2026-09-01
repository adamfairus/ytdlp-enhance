use dlp::lyrics::LyricsFetcher;

#[test]
fn test_clean_title_for_lyrics_search() {
    assert_eq!(
        LyricsFetcher::clean_title("ICONIC HEART [Official Music Video]"),
        "ICONIC HEART"
    );
    assert_eq!(
        LyricsFetcher::clean_title("Never Gonna Give You Up (Official Video)"),
        "Never Gonna Give You Up"
    );
    assert_eq!(
        LyricsFetcher::clean_title("Song Title (MV)"),
        "Song Title"
    );
    assert_eq!(
        LyricsFetcher::clean_title("01 - Accendio (Official MV)"),
        "Accendio"
    );
    assert_eq!(
        LyricsFetcher::clean_title("02. HEYA [Remastered]"),
        "HEYA"
    );
}

#[test]
fn test_clean_artist_for_lyrics_search() {
    assert_eq!(
        LyricsFetcher::clean_artist("Hearts2Hearts - Topic"),
        "Hearts2Hearts"
    );
    assert_eq!(
        LyricsFetcher::clean_artist("Singer A, Singer B"),
        "Singer A"
    );
}
