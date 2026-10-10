use std::{
    path::{Path, PathBuf},
    iter::repeat_n,
    fs,
};
use anyhow::{Context, ensure};
use directories::BaseDirs;

use crate::sudo_cmd;
use crate::block_device::{BlockDevice, PartitionPaths, mock_sd};
use crate::manifest;


pub fn image_cache_dir() -> anyhow::Result<PathBuf> {

    let dirs = BaseDirs::new()
        .context("could not determine user directories")?;

    let image_dir = dirs
        .cache_dir()
        .join("siali")
        .join("images");

    fs::create_dir_all(&image_dir)?;

    Ok(image_dir)
}


fn foundation_archive_dir() -> anyhow::Result<PathBuf> {

    let basedirs = BaseDirs::new()
        .context("Could not determine user directories")?;

    let ret_dir =
        basedirs.cache_dir()
                .join("siali")
                .join("foundations");

    Ok(ret_dir)
}


fn foundation_archive_path(sbc_model: &str) -> anyhow::Result<PathBuf> {

    let foundation_archive_name = manifest::read(sbc_model)?
        .foundation_archive_name()?;

    let foundation_archive_path = foundation_archive_dir()?
            .join(foundation_archive_name);

    Ok(foundation_archive_path)
}


/// Create a .img of selected Arch Linux ARM filesystem that we call a
/// _foundation_, hence it is a foundation .img. This adapts these installation
/// instructions: https://archlinuxarm.org/platforms/armv8/broadcom/raspberry-pi-zero-2,
/// which are essentially the same for the RPi 3 and 4. This also works as
/// the first step to set up the RPi 5, but additional kernel tweaks are needed
/// for the 5. This may fail for other Pi-style single-board computers, but
/// we do not yet produce any boxes based on these, and so we will deal with
/// any such issues when we get there.
///
/// It is created in the following steps:
///   1. Fetch the foundation filesystem online, eg, from
///      http://os.archlinuxarm.org/os/ArchLinuxARM-rpi-aarch64-latest.tar.gz.
///   2. Unarchive the filesystem directly onto a loop device that has been
///      partitioned with filesystems assigned as described Loop devices are
///      files that the operating system treats like a block storage device,
///      i.e., to mock an SD card.
///   3. Mount the boot partition on the same loop device to the user space
///      file system, then move everything from the /boot/ directory in the
///      unpacked linux filesystem to the newly-mounted boot partition.
///   4. Sync & unmount loop device partitions and .img file.
///
pub fn create_foundation_img(
        sbc_model: &str,
        mock_sd_size_mib: u64,
        boot_size_mib: u64,
    ) -> anyhow::Result<PathBuf> {

    let image_path = image_cache_dir()?
        .join(format!("{sbc_model}.img"));

    //--- *** Step 1:
    // Create a mock block storage device to copy the OS filesystem
    let mut device = mock_sd(&image_path, mock_sd_size_mib)?;
    // Partition the device
    device.create_partitions(boot_size_mib)?;

    let device_path = device.path();

    let mut boot_partition = device_path.as_os_str().to_owned();
    boot_partition.push("p1");
    let boot_partition = PathBuf::from(boot_partition);

    let mut root_partition = device_path.as_os_str().to_owned();
    root_partition.push("p2");
    let root_partition = PathBuf::from(root_partition);

    let partition_paths = PartitionPaths {
        boot: boot_partition,
        root: root_partition
    };

    // Create
    device.format_partitions(&partition_paths)?;

    // Load the path to the foundation .tar.gz with linux filesystem
    let foundation_path = foundation_archive_path(sbc_model)?;

    // If it doesn't exist, bail with instructions for acquisition
    if !foundation_path.is_file() {
        anyhow::bail!(
            "Foundation archive not found: {}. Try `siali fetch {}.",
            foundation_path.display(), sbc_model
        )
    }

    // Create a temporary directory where loop device partitions will be mounted
    let mount_dir = tempfile::tempdir()?;

    // Create paths to the two subdirectories of the temporary directory
    let mount_root = mount_dir.path().join("root");
    let mount_boot = mount_dir.path().join("boot");

    // Create the directories in the userspace file system, in the temp dir
    fs::create_dir_all(&mount_root)?;
    fs::create_dir_all(&mount_boot)?;

    mount(&partition_paths.root, &mount_root)?;
    extract_foundation_to_root(&foundation_path, &mount_root)?;


    //--- *** Step 3: copy everything from boot dir in mounted root to boot part
    //
    // We need the boot/ files in the first partition from the new OS filesystem
    let device_boot: PathBuf = partition_paths.boot;
    // We need the device boot partition mounted
    mount(&device_boot, &mount_boot)?;
    // Copy the boot files from the root dir in user space to boot partition
    move_boot_files(&device_boot, &mount_root, &mount_boot)?;

    //--- *** Step 4: sync and unmount devices
    //
    // First sync, remove the loop device mounts from the user space filesystem
    sudo_cmd("sync")
        .status()
        .with_context(|| "Failed to sync filesystem after OS extraction")?;

    unmount(&device_boot)?;
    unmount(&partition_paths.root)?;

    sudo_cmd("sync")
        .status()
        .with_context(|| "Failed to sync filesystem after OS extraction")?;

    // Detach the device from the .img acting like the SD card; install remains
    device.detach();

    Ok(image_path)
}


fn mount(device_partition: &Path,
         mountpoint: &Path) -> anyhow::Result<()> {

    fs::create_dir_all(mountpoint)
        .with_context(|| format!("Count not create mountpoint {}",
                                 mountpoint.display()))?;

    let status = sudo_cmd("mount")
        .arg(device_partition)
        .arg(mountpoint)
        .status()
        .with_context(|| {
            format!("Count not mount {} at {}",
                    device_partition.display(),
                    mountpoint.display())
        })?;

    ensure!(
        status.success(),
        "Count not mount {} at {}",
        device_partition.display(),
        mountpoint.display()
    );

    Ok(())
}


fn unmount(mountpoint: &Path) -> anyhow::Result<()> {

    sudo_cmd("umount")
        .arg(mountpoint)
        .status()
        .with_context(
            || format!("Failed to unmount {}", mountpoint.display())
        )?;

    Ok(())
}


fn extract_foundation_to_root(foundation_path: &PathBuf,
                              mount_root: &PathBuf) ->
    anyhow::Result<()> {

    // eg $ sudo bsdtar -xpf ArchLinuxARM-rpi-armv7-latest.tar.gz -C root
    let foundation_display = foundation_path.display();
    let mount_display = mount_root.display();

    sudo_cmd("bsdtar")
        .arg("-xpf")
        .arg(format!("{}", foundation_display))
        .arg("-C")
        .arg(format!("{}", mount_display))
        .status()
        .with_context(|| format!("Failed to extract {} to {}",
                                 foundation_display, mount_display))?;

    sudo_cmd("sync")
        .status()
        .with_context(|| "Failed to sync filesystem after OS extraction")?;

    Ok(())
}


/// Move all entries from the new filesystem's boot directory to the mounted
/// boot partition. These are the files the SBC reads to initialize the
/// hardware so the software can operate as desired.
fn move_boot_files(
    device_boot: &PathBuf,
    mount_root: &PathBuf,
    mount_boot: &PathBuf,
) -> anyhow::Result<()> {
    // Mount the device boot partition outside the mounted root filesystem.
    mount(device_boot, mount_boot)?;

    // Get the boot directory in the new mounted OS filesystem.
    let boot_to_be_moved = mount_root.join("boot");

    ensure!(
        boot_to_be_moved.exists() && boot_to_be_moved.is_dir(),
        "Boot dir {} does not exist to copy to boot partition",
        boot_to_be_moved.display()
    );

    // Move each entry, equivalent to `mv mount_root/boot/* mount_boot`.
    // Expand the entries in Rust; Command arguments do not expand shell globs.
    for entry in fs::read_dir(&boot_to_be_moved)
        .with_context(|| format!("Failed to read {}", boot_to_be_moved.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        let status = sudo_cmd("mv")
            .arg("--")
            .arg(&path)
            .arg(mount_boot)
            .status()
            .with_context(|| {
                format!(
                    "Failed to move {} into {}",
                    path.display(),
                    mount_boot.display()
                )
            })?;

        ensure!(status.success(), "mv failed with {status}");
    }

    Ok(())
}


pub fn compress(
    image_path: &Path,
    threads: u32,
    level: i64,
    memory_limit: &str,
    verbose: u8,
    overwrite: bool
) -> anyhow::Result<PathBuf> {


    if !image_path.is_file() {
        anyhow::bail!("image not found: {}", image_path.display());
    }

    // Use the `xz` program to compress the image file
    let xz_status =
        sudo_cmd("xz")
            .arg("--keep")
            .arg(format!("-T{threads}"))
            .arg(format!("-{level}"))
            .arg(format!("--memlimit-compress={memory_limit}"))
            .args(overwrite.then_some("--force"))
            .args(repeat_n("-v", verbose.into()))
            .arg(image_path)
            .status()
            .with_context( || { format!("xz failed to run") } )?;

    if !xz_status.success() {
        anyhow::bail!(
            "xz tried to compress () but had an error, status: {xz_status}"
        )
    }

    let compressed_path =
        PathBuf::from(format!("{}.xz", image_path.display()));

    Ok(compressed_path)
}
