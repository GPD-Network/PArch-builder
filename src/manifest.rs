//! Defines and reads platform manifests.
//!
//! Manifests describe the inputs needed to build a target platform,
//! beginning with its foundation filesystem.
use std::{
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::Deserialize;
use url::Url;


#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub sbc_model: String,
    pub foundation_url: Url,
}


/// Represent the SBC model manifest
impl Manifest {

    pub fn foundation_archive_name(self) -> anyhow::Result<String> {

        self.foundation_url
            .path_segments()
            .and_then(|segments| segments.last())
            .filter(|name| !name.is_empty())
            .context("URL has no filename")
            .map(str::to_owned)
    }
}


/// Returns the platform-standard configuration directory for manifests
pub fn manifest_dir() -> Result<PathBuf> {
    let basedirs = BaseDirs::new()
        .context("Could not determine user directories")?;

    let ret_dir =
        basedirs.config_dir()
                .join("siali")
                .join("manifests");


    Ok(ret_dir)
}


/// Returns the platform-standard path for a named manifest.
///
/// ```text
/// Linux:  ~/.config/siali/manifests/rpi2w.yml
/// macOS:  ~/Library/Application Support/siali/manifests/rpi2w.yml
/// Windows: C:\Users\Alice\AppData\Roaming\siali\manifests\rpi2w.yml
/// ```
///
/// The exact base directory may vary according to environment variables
/// and operating-system configuration.
///
/// # Example
///
/// ```rust
/// use std::path::Path;
/// use siali::manifest::manifest_path;
///
/// let path = manifest_path("rpi2w")?;
///
/// assert!(
///     path.ends_with(
///         Path::new("siali")
///             .join("manifests")
///             .join("rpi2w.yml")
///     )
/// );
///
///  Ok(())
/// ```
pub fn manifest_path(name: &str) -> Result<PathBuf> {
    Ok(manifest_dir()?.join(format!("{name}.yml")))
}


pub fn read(sbc_model: &str) -> Result<Manifest> {

    // Build the path to the manifest file based on given name
    let path = manifest_path(sbc_model)?;

    // Read manifest YAML into memory
    let contents = fs::read_to_string(&path)
        .with_context(
            || format!(
                "Could not read manifest {}", path.display()
            )
        )?;

    // Parse manifest contents, returning result as Manifest thru introspection
    serde_yaml::from_str(&contents)
        .with_context(
            || format!(
                "Could not parse manifest {}",
                path.display()
            )
        )
}
