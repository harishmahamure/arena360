fn main() {
    #[cfg(windows)]
    ensure_bootstrapper_resource();
    tauri_build::build();
}

#[cfg(windows)]
fn ensure_bootstrapper_resource() {
    use std::path::PathBuf;

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
