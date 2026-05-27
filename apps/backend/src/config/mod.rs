mod database;
mod settings;

pub use database::{create_pool, ping};
pub use settings::Settings;

/// Load `.env` from the crate directory (`apps/backend/.env`), then fall back to cwd.
/// Required when running via `pnpm backend:dev` from the monorepo root.
pub fn load_dotenv() {
    let manifest_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if dotenvy::from_path(&manifest_env).is_ok() {
        return;
    }
    dotenvy::dotenv().ok();
}
