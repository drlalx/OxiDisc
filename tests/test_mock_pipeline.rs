#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::AtomicBool;

#[test]
fn test_mock_cd_toc_and_discid_consistency() {
    let toc = oxidisc::mock_cd::get_mock_toc("mock").expect("Mock TOC should be valid");

    assert_eq!(toc.track_count(), 6);
    assert_eq!(toc.first_track, 1);
    assert_eq!(toc.last_track, 6);
    assert_eq!(toc.leadout_offset, 95462);

    // Official MusicBrainz DiscID for this TOC layout
    let mbid = toc.calculate_disc_id();
    assert_eq!(mbid, "49HHV7Eb8UKF3aQiNmu1GR8vKTY-");

    // CDDB/FreeDB/GnuDB DiscID for this TOC layout
    let cddb_id = toc.cddb_disc_id_string();
    assert_eq!(cddb_id, "3404f606");
}

#[test]
fn test_mock_cd_pcm_audio_generation() {
    let cancel = AtomicBool::new(false);
    let mut pcm_buffer = Vec::new();
    let mut progress_calls = 0;

    let res = oxidisc::mock_cd::rip_sectors_mock(
        150,
        350,
        &cancel,
        |_progress| {
            progress_calls += 1;
        },
        &mut pcm_buffer,
    );

    assert!(res.is_ok());
    // 200 sectors * 2352 bytes = 470,400 bytes of 16-bit 44.1kHz stereo PCM
    assert_eq!(pcm_buffer.len(), 200 * 2352);
    assert!(progress_calls > 0);

    // Verify samples are non-silent
    let non_zero_samples = pcm_buffer.iter().filter(|&&b| b != 0).count();
    assert!(non_zero_samples > pcm_buffer.len() / 2);
}

#[test]
fn test_mock_cd_acoustid_fingerprinting() {
    // Generate 15 seconds of mock PCM samples
    let samples = oxidisc::mock_cd::generate_mock_track_samples(1, 15);
    assert_eq!(samples.len(), 15 * 44100 * 2);

    let fp = oxidisc::acoustid::AcoustidClient::fingerprint_pcm16le(&samples, 44100, 2);
    assert!(fp.is_ok());
    assert!(!fp.unwrap().is_empty());
}

#[test]
fn test_mock_cd_offline_fallback_metadata() {
    let toc = oxidisc::mock_cd::get_mock_toc("mock").unwrap();
    let info = oxidisc::mock_cd::get_mock_fallback_info(&toc);

    assert_eq!(info.title, "Synthetic Test Album");
    assert_eq!(info.artist, "OxiDisc Audio Lab");
    assert_eq!(info.tracks.len(), 6);
    assert_eq!(info.tracks[0], "Overture in C Major");
    assert_eq!(info.disc_id, "49HHV7Eb8UKF3aQiNmu1GR8vKTY-");
}

#[test]
fn test_mock_cd_device_detection() {
    assert!(oxidisc::mock_cd::is_mock_device("mock"));
    assert!(oxidisc::mock_cd::is_mock_device("test"));
    assert!(oxidisc::mock_cd::is_mock_device("mock:fast"));

    let toc = oxidisc::cd_reader::CdReader::read_disc_toc("mock");
    assert!(toc.is_ok());
    assert_eq!(toc.unwrap().track_count(), 6);
}

#[test]
fn test_auto_create_output_dir_configuration() {
    let mut cfg = oxidisc::config::Config::default();
    assert!(!cfg.auto_create_output_dir);

    cfg.auto_create_output_dir = true;
    let serialized = toml::to_string(&cfg).unwrap();
    assert!(serialized.contains("auto_create_output_dir = true"));

    let deserialized: oxidisc::config::Config = toml::from_str(&serialized).unwrap();
    assert!(deserialized.auto_create_output_dir);
}

#[test]
fn test_encoder_availability_checks() {
    // WAV and FLAC must always be available (100% pure Rust)
    assert!(oxidisc::ripper::check_encoder_tool_available("wav").is_ok());
    assert!(oxidisc::ripper::check_encoder_tool_available("flac").is_ok());
}

#[test]
fn test_mock_rip_flac_pure_rust_end_to_end() {
    let temp_dir = std::env::temp_dir().join("oxidisc_test_flac_rip");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut cfg = oxidisc::config::Config::default();
    cfg.encoder = "flac".to_string();
    cfg.device = "mock".to_string();

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let ripper = oxidisc::ripper::Ripper::new(cfg, temp_dir.clone(), sender).unwrap();

    let toc = oxidisc::mock_cd::get_mock_toc("mock").unwrap();
    let mut info = oxidisc::mock_cd::get_mock_fallback_info(&toc);
    // Rip only track 1 for fast test
    info.tracks.truncate(1);

    ripper.rip(&info);

    let mut success = false;
    while let Ok(msg) = receiver.try_recv() {
        if matches!(msg, oxidisc::ripper::RipMessage::Success) {
            success = true;
        }
    }
    assert!(success, "Ripping to FLAC in pure Rust must succeed");

    let album_dir = temp_dir.join("Synthetic Test Album");
    let expected_flac = album_dir.join("Overture in C Major.flac");
    assert!(expected_flac.exists(), "FLAC file must be created on disk");
    assert!(
        expected_flac.metadata().unwrap().len() > 1000,
        "FLAC file must not be empty"
    );

    // Ensure temp wav file was removed
    assert!(
        !album_dir.join("track01.wav").exists(),
        "Temporary WAV must be cleaned up"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
