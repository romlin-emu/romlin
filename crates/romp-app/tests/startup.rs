#![cfg(target_os = "linux")]

use std::process::Command;

#[test]
fn startup_failures_are_written_to_the_app_log() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_romlin"))
        .env("ROMLIN_DATA_DIR", dir.path())
        .env("RUST_LOG", "info")
        .env("SLINT_BACKEND", "winit-software")
        .env("WINIT_UNIX_BACKEND", "x11")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let log = std::fs::read_to_string(dir.path().join("logs/romlin.log")).unwrap();
    assert!(log.contains("Romlin failed"), "{log}");
    assert!(
        log.contains("DISPLAY"),
        "the error cause must be logged: {log}"
    );
}
