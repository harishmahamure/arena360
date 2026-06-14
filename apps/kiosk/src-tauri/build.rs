fn main() {
    // prepare-watchdog-sidecar.mjs builds arena360-watchdog before the sidecar file
    // exists at externalBin path; skip validation for that cargo invocation only.
    if std::env::var("SKIP_TAURI_EXTERNAL_BINS").is_ok() {
        return;
    }
    tauri_build::build()
}
