pub mod fetch;
pub mod manifest;
pub mod block_device;
pub mod image;
pub mod sd;

use std::path::PathBuf;
use std::process::Command;


pub fn sudo_cmd(cmd_name: &str) -> std::process::Command {
    let mut sudo_cmd = Command::new("sudo");
    sudo_cmd.arg(cmd_name);

    sudo_cmd
}


pub fn read_manifest_and_fetch_foundation(
    platform: &str,
    overwrite: bool) -> anyhow::Result<PathBuf> {


    // Fetch the foundation URL field stored in the manifest of interest
    let foundation_path = fetch::fetch(
        manifest::read(platform)?.foundation_url,
        overwrite
    )?;

    Ok(foundation_path)
}


// *** LIBRARY INTEGRATION TESTS ***
//
#[cfg(test)]
mod tests {

    use super::*;
    use anyhow::{Context,Result};
    use directories::BaseDirs;
    use std::io::Write;
    use tempfile::Builder;
    use std::path::Path;

    use crate::manifest::manifest_path;

    #[test]
    fn manifest_path_expected_location_and_format() -> Result<()> {

        let path = manifest_path("rpi2w")?;

        assert!(
            path.ends_with(
                Path::new("parch-builder")
                    .join("manifests")
                    .join("rpi2w.yml")
            )
        );

        Ok::<(), anyhow::Error>(())
    }

    #[test]
    fn reads_manifest_and_fetches_foundation() -> Result<()> {

        // Create XDG manifest directory if it doesn't already exist
        let manifest_dir = manifest::manifest_dir()?;
        std::fs::create_dir_all(&manifest_dir)?;

        // Create mock manifest in that directory for collision resistance
        let mut manifest_file = Builder::new()
            .prefix("pb-integration-")
            .suffix(".yml")
            .tempfile_in(&manifest_dir)?;

        writeln!(
            manifest_file,
            "\
platform: rust-logo
foundation_url: https://rust-lang.org/static/images/rust-logo-blk.svg"
        )?;

        // Flush the manifest file buffer to finish writing
        manifest_file.flush()?;

        // *** We need to replace .svg with .yml in our temp manifest file name
        // Start by extracting only the "platform" including temp file slug
        let platform = manifest_file
            .path()
            .file_stem()
            .context("Temporary manifest has no file stem")?
            .to_str()
            .context("Temporary manifest filename is not valid UTF-8")?;

        let dirs = BaseDirs::new()
            .context("Could not determine user directories")?;

        // Find the "foundation" we downloaded
        let downloaded_foundation = dirs
            .cache_dir()
            .join("parch-builder")
            .join("foundations")
            .join("rust-logo-blk.svg");

        // Test helper function for the `fetch` cli command
        read_manifest_and_fetch_foundation(
            platform,
            true  // overwrite
        )?;

        assert!(downloaded_foundation.is_file());
        assert!(downloaded_foundation.metadata()?.len() > 0);


        // Now replace original contents of mock "yml" (actually SVG) to track
        let marker = b"overwrite=false shouldn't overwrite this...";
        std::fs::write(
            &downloaded_foundation,
            marker
        )?;

        // Test that overwrite=false preserves the existing file
        read_manifest_and_fetch_foundation(
            platform,
            false  // don't overwrite
        )?;

        assert_eq!(
            std::fs::read(&downloaded_foundation)?.as_slice(),
            marker
        );

        // Clean up downloaded test "foundation"
        std::fs::remove_file(downloaded_foundation)?;

        // manifest_file is automatically removed at the end of its scope
        Ok(())
    }

}
