use anyhow::{Context, Result};
use dissimulare_platform::AppPaths;

/// Loads the per-installation chaos seed, generating and persisting a new
/// random one on first use. Stable across restarts so a given domain keeps
/// seeing the same absurd identity; unique per installation so no two
/// Dissimulare users hand out the same pattern.
pub fn load_or_generate_chaos_seed(paths: &AppPaths) -> Result<Vec<u8>> {
    let path = paths.chaos_seed_file();

    if let Ok(bytes) = std::fs::read(&path) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|err| anyhow::anyhow!("generating chaos seed: {err}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, seed).with_context(|| format!("writing {}", path.display()))?;

    Ok(seed.to_vec())
}
