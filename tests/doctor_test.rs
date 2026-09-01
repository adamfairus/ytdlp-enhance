use dlp::config::Config;
use dlp::doctor::Doctor;
use dlp::preset::PresetManager;

#[test]
fn test_doctor_diagnostics_runs_cleanly() {
    let config = Config::default();
    let manager = PresetManager::load_all();
    let res = Doctor::run_diagnostics(&manager, &config);
    assert!(res.is_ok());
}
