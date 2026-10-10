use std::io;
use std::io::Write;  // Write trait provides the `flush` method
use std::path::Path;

use anyhow::Context;

use crate::sudo_cmd;


pub fn install_img(platform: &str, device: &Path) -> anyhow::Result<()> {

    // Create path to the cached .img for specified platform
    let image_path = crate::image::image_cache_dir()?
        .join(format!("{}.img", platform));

    // Form the dd command to write the desired platform img to spec'd device
    println!("Now writing image {} to {}", image_path.display(), device.display());

    // Confirm user indeed wants to install the image to the device
    print!("Install {image_path:?} to {device:?}? [y/N] ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;

    // Exit with return code 0 if user disconfirms
    if !answer.trim().eq_ignore_ascii_case("y") {
        println!("Cancelled.");
        return Ok(());
    }

    // Run dd command
    sudo_cmd("dd")
        .arg(format!("if={}", image_path.display()))
        .arg(format!("of={}", device.display()))
        .args(["bs=4M", "status=progress", "conv=fsync"])
        .status()
        .context(
            format!("Failed to write image at {} to device {}",
                    image_path.display(), device.display())
        )?;

    // Expand the partition beyond the image written to the SD, always 2
    println!("Now growing root partition to whole disk");
    sudo_cmd("growpart")
        .arg(device)
        .arg("2")
        .status()
        .context("Failed to grow partition")?;

    // Repair disk if necessary
    println!("Running e2fsck to fix inconsistencies...");
    let root_device = format!("{}2", device.display()).to_string();
    sudo_cmd("e2fsck")
        .arg("-f")
        .arg(&root_device)
        .status()
        .context("Failed to inspect and repair root partition")?;

    // Finalize by creating filesystem on the resized disk
    println!(
        "Root partition healthy. Final step: make a new filesystem on the resized partition."
    );

    sudo_cmd("resize2fs")
        .arg(&root_device)
        .status()
        .context("Failed to set filesystem on root partition")?;

    Ok(())
}
