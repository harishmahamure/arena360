use std::path::PathBuf;

fn main() {
    ensure_watchdog_resource_stub();
    #[cfg(windows)]
    {
        ensure_bootstrapper_resource();
        build_and_stage_watchdog();
    }
    tauri_build::build();
}

#[cfg(not(windows))]
fn ensure_watchdog_resource_stub() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let resources_dir = manifest_dir.join("resources");
    let dest = resources_dir.join("arena360-watchdog.exe");
    if dest.is_file() {
        return;
    }
    std::fs::create_dir_all(&resources_dir).expect("create resources dir");
    std::fs::write(&dest, []).expect("write watchdog resource stub");
}

#[cfg(windows)]
fn build_and_stage_watchdog() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let watchdog_manifest = manifest_dir.join("../watchdog/Cargo.toml");

    println!("cargo:rerun-if-changed=../watchdog");
    println!("cargo:rerun-if-changed=../watchdog-common");

    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            watchdog_manifest.to_str().expect("watchdog manifest path"),
            "--profile",
            &profile,
        ])
        .status()
        .expect("spawn cargo build for arena360-watchdog");

    if !status.success() {
        panic!("arena360-watchdog build failed with status {status}");
    }

    let built = manifest_dir
        .join("../watchdog/target")
        .join(&profile)
        .join("arena360-watchdog.exe");

    let resources_dir = manifest_dir.join("resources");
    std::fs::create_dir_all(&resources_dir).expect("create resources dir");
    let dest = resources_dir.join("arena360-watchdog.exe");
    std::fs::copy(&built, &dest).unwrap_or_else(|e| {
        panic!(
            "copy watchdog binary from {} to {}: {e}",
            built.display(),
            dest.display()
        )
    });
    println!("cargo:rerun-if-changed={}", dest.display());
}

#[cfg(windows)]
fn ensure_bootstrapper_resource() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let resources_dir = manifest_dir.join("resources");
    let dest = resources_dir.join("MicrosoftEdgeWebview2Setup.exe");

    if dest.is_file() {
        return;
    }

    eprintln!(
        "kiosk build: downloading WebView2 Evergreen bootstrapper to {}",
        dest.display()
    );

    std::fs::create_dir_all(&resources_dir).expect("create resources dir");

    const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("build reqwest client for WebView2 bootstrapper");

    let bytes = client
        .get(BOOTSTRAPPER_URL)
        .send()
        .expect("download WebView2 bootstrapper")
        .error_for_status()
        .expect("WebView2 bootstrapper download HTTP status")
        .bytes()
        .expect("read WebView2 bootstrapper body");

    std::fs::write(&dest, &bytes).expect("write WebView2 bootstrapper");
    eprintln!(
        "kiosk build: saved WebView2 bootstrapper ({} bytes)",
        bytes.len()
    );
}
