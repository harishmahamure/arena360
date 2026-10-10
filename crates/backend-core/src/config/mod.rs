mod database;
mod roles;
mod settings;

pub use database::{create_pool_for, ping};
pub use roles::Roles;
pub use settings::Settings;

/// Load the repository root `.env`, then the legacy backend `.env`, then cwd.
/// Exported process variables always take precedence.
pub fn load_dotenv() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in [root.join(".env"), root.join("apps/backend/.env")] {
        if dotenvy::from_path(path).is_ok() {
            return;
        }
    }
    dotenvy::dotenv().ok();
}
