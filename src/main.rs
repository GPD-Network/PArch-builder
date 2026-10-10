//! Builder for Arch Linux on Pi-style and Generic Platforms.
use std::os::unix::fs::FileTypeExt;  // Check if path is block device, e.g.
use std::path::{PathBuf};

// Third-party cargo imports
use clap::{Parser, Subcommand};
use glob::glob;

// Library imports
use siali::{manifest,image};


#[derive(Parser)]
#[command(
    name = "siali",
    version,
    about = "Build reproducible Sustained Inference Arch Linux, SÍ-ALí, system images from configuration manifests"
)]


struct Cli {
    #[command(subcommand)]
    command: Commands,
}


#[derive(Subcommand)]
enum Commands {

    /// List available build targets
    List,

    /// Fetch a source from the manifest
    Fetch {
        /// platform name indicating yml in manifests
        platform: String,

        /// Whether to overwrite existing foundations
        #[arg(long, short='o')]
        overwrite: bool,
    },

    /// Build an img of the foundation for the given platform
    Build {
        /// Short name of the platform matching manifest yml, eg, rpi2w
        platform: String,
        ///
        /// Size of the virtual SD card for making the .img in MiB
        #[arg(long, default_value_t=8_000)]
        mock_sd_size_mib: u64,

        /// Size of the boot partition in MiB
        #[arg(long, default_value_t=512)]
        boot_size_mib: u64,

        /// Whether to overwrite any existing foundation img
        #[arg(long, short='o')]
        overwrite: bool,
    },

    /// Compress a raw .img image with xz
    Compress {
        /// Path to the raw .img file
        image_path: PathBuf,

        /// Number of worker threads for xz comp; 0 sets number of threads automatically
        #[arg(short = 'T', long, default_value_t = 0)]
        threads: u32,

        /// xz compression level from 0 (least compression) to 9 (most)
        #[arg(short, long, default_value_t = 9, value_parser = 0..=9)]
        level: i64,

        /// Compression memory limit, set to 75% of available RAM by default
        #[arg(short = 'M', long, default_value = "75%")]
        memory_limit: String,

        /// Add xz verbosity; -v gives detailed diagnostics.
        #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 2)]
        verbose: u8,

        /// Flag to overwrite existing compressed file
        #[arg(long, short = 'o')]
        overwrite: bool
    },

    /// Install a parch platform to a device
    Install {

        /// Short name for platform, i.e. x86 or SBC. Eg, rpi2w for the Raspberry Pi Zero 2W
        platform: String,

        /// Device where siali is to be installed
        device_path: PathBuf,

    },
}


fn main() -> anyhow::Result<()> {

    let cli = Cli::parse();

    match cli.command {
        // *** LIST ***
        Commands::List => {

            let manifest_dir = manifest::manifest_dir()?;

            let manifest_path = manifest_dir.as_path();

            // Buiild glob string in three steps to respect borrowing
            let mut manifest_glob_str = String::new();

            manifest_glob_str.push_str(
                manifest_path
                    .to_str()
                    .unwrap_or_else(
                        || ".config/siali/manifests"
                    )
            );

            manifest_glob_str.push_str("/*.yml");

            println!("\nAvailable platform manifests found with glob\n{}:\n\n",
                     manifest_glob_str);

            for entry in glob(&manifest_glob_str)
                .expect("Failed to read manifest.")
            {
                match entry {
                    Ok(path) => println!("{:?}", path.display()),
                    Err(e) => eprintln!("{}", e)
                }
            }

            Ok(())
        }

        // *** FETCH ***
        Commands::Fetch {
            platform,
            overwrite,
        } => {

            println!("Fetching the foundation for platform {}...", platform);

            let foundation_path = siali::read_manifest_and_fetch_foundation(
                &platform, overwrite
            )?;

            // Notify user what was done
            println!(
                "Foundation archive acquired for SBC model {}.", platform
            );
            println!(
                "Foundation archive has been synced to {}", foundation_path.display()
            );

            Ok(())
        }

        // *** BUILD ***
        Commands::Build {
            platform,
            mock_sd_size_mib,
            boot_size_mib,
            overwrite,
        } => {

            let image_path = image::image_cache_dir()?
                .join(format!("{platform}.img"));

            if image_path.exists() && !overwrite {
                eprintln!("Using cached image: {}", image_path.display());
                return Ok(());
            }

            let image_path = siali::image::create_foundation_img(
                &platform,
                mock_sd_size_mib,
                boot_size_mib
            )?;

            println!("Arch Linux ARM image available at {}",
                image_path.display());

            Ok(())
        },

        Commands::Compress {
            image_path,
            threads,
            level,
            memory_limit,
            verbose,
            overwrite
        } => {

            let compressed_path = PathBuf::from(
                format!("{}.xz", image_path.display())
            );

            if compressed_path.is_file() && !overwrite {

                println!(
                    "\n\nCompressed image already exists at {}, and overwrite is false",
                    compressed_path.display()
                );

                return Ok(())
            }

            let compressed_path =
                siali::image::compress(
                    &image_path,
                    threads,
                    level,
                    &memory_limit,
                    verbose,
                    overwrite
                )?;

            println!(
                "\n\nCompressed image created at {}",
                compressed_path.display()
            );

            Ok(())
        },

        // *** INSTALL TO CARTÕES ***
        Commands::Install { platform, device_path  } => {

            // Read device metadata to confirm it is a block device; bail if not
            let metadata = std::fs::metadata(&device_path)?;
            if !metadata.file_type().is_block_device() {
                anyhow::bail!("Not a block device: {}", device_path.display());
            }

            siali::sd::install_img(&platform, &device_path)?;

            print!("\nInstallation complete.\n\nInstalled {platform:?} to {device_path:?}.\n\n");

            Ok(())
        }
    }
}

