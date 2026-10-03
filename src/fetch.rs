//! Fetches and caches foundation filesystem archives.
use std::path::PathBuf;
use std::fs::File;

use anyhow::{Context, Result};
use directories::BaseDirs;
use indicatif::{ProgressBar, ProgressStyle};


fn foundation_cache_dir() -> Result<PathBuf> {

    let dirs = BaseDirs::new()
        .context("Could not determine user directories")?;

    let cache_dir =
        dirs.cache_dir()
            .join("siali")
            .join("foundations");

    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir)?;
    }

    Ok(cache_dir)
}


/// Fetch the resource at the given URL and save. The user must explicitly
/// specify if they would like to overwrite it locally if it exists already.
pub fn fetch(
        url: reqwest::Url,
        overwrite: bool
    ) -> anyhow::Result<PathBuf> {

    // Now split URL to take last part (filename) and append to savepath
    let filename = url
        .path_segments()
       .and_then(|mut segments| segments.next_back())
       .expect("Reading filename from download URL failed. Check URL.");

    // Construct system save path from save dir and the file name from the URL
    let savepath = foundation_cache_dir()?.join(filename);

    // If the save path exists already and overwrite is disabled, do nothing
    if savepath.exists() && !overwrite {
        eprintln!("Using cached foundation: {}", savepath.display());
        return Ok(savepath);
    }

    // Get the payload to write to file
    let response = reqwest::blocking::get(
        url.as_str()
    )?.error_for_status()?;

    let progress = match response.content_length() {
        Some(length) => ProgressBar::new(length),
        None => ProgressBar::no_length(),
    };


    progress.set_style(
    ProgressStyle::with_template(
        "\x1b[36m{spinner} [\x1b[0m\
         {bar:40.cyan/blue}\
         \x1b[36m] {bytes}/{total_bytes} \
         ({bytes_per_sec}, {eta})\x1b[0m"
    )?
    .progress_chars("•  "),
);

    let mut source = progress.wrap_read(response);
    let mut destination = File::create(&savepath)?;

    std::io::copy(
        &mut source,
        &mut destination,
    )?;

    progress.finish_with_message("Downloaded");

    Ok(savepath)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetches_rust_logo() -> Result<()> {

        let url = reqwest::Url::parse(
            "https://www.rust-lang.org/static/images/rust-logo-blk.svg"
        )?;

        let path = foundation_cache_dir()?
            .join("rust-logo-blk.svg");

        let existed_before = path.exists();

        let fetched_path: PathBuf = fetch(url, false)?;
        assert!(fetched_path.is_file());
        assert!(fetched_path.metadata()?.len() > 0);

        assert!(path.is_file());
        assert!(path.metadata()?.len() > 0);

        // Don't remove a file that existed before this test.
        if !existed_before {
            std::fs::remove_file(path)?;
        }

        Ok(())
    }
}
