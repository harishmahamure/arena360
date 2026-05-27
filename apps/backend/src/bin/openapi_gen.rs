use std::{env, fs, path::PathBuf};

use gaming_cafe_api::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_dir = manifest_dir.join("docs");
    fs::create_dir_all(&docs_dir)?;

    let spec = ApiDoc::openapi().to_pretty_json()?;
    let output = docs_dir.join("openapi.json");
    fs::write(&output, spec)?;

    println!("OpenAPI spec written to {}", output.display());
    Ok(())
}
