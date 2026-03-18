use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use dicom_object::open_file;
use dicom_pixeldata::PixelDecoder;
use image::ImageEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use rayon::prelude::*;

#[derive(Parser)]
#[command(name = "dicom2png", about = "Convert DICOM (.dcm) files to PNG")]
struct Cli {
    /// Input path: a .dcm file, a directory of .dcm files, or a parent directory containing subdirectories of .dcm files
    input: PathBuf,

    /// Output directory (defaults to "output")
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Disable PNG compression (faster, larger files)
    #[arg(long)]
    no_compression: bool,
}

/// A conversion job: input .dcm path and the output .png path.
struct Job {
    input: PathBuf,
    output: PathBuf,
}

fn collect_jobs(input: &Path, output: &Path) -> Vec<Job> {
    if input.is_file() {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        return vec![Job {
            input: input.to_path_buf(),
            output: output.join(format!("{stem}.png")),
        }];
    }

    let mut jobs = Vec::new();

    let Ok(entries) = fs::read_dir(input) else {
        return jobs;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("dcm") {
            let out_name = format!(
                "{}.png",
                path.file_stem().unwrap_or_default().to_string_lossy()
            );
            jobs.push(Job {
                input: path,
                output: output.join(out_name),
            });
        } else if path.is_dir() {
            // Subdirectory: preserve folder name in output
            let folder_name = path.file_name().unwrap_or_default();
            let sub_output = output.join(folder_name);
            jobs.extend(collect_jobs(&path, &sub_output));
        }
    }

    jobs
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let no_compression = cli.no_compression;

    let jobs = collect_jobs(&cli.input, &cli.output);

    if jobs.is_empty() {
        eprintln!("No .dcm files found in {:?}", cli.input);
        std::process::exit(1);
    }

    // Create all needed output directories upfront
    let mut dirs: Vec<&Path> = jobs.iter().filter_map(|j| j.output.parent()).collect();
    dirs.sort();
    dirs.dedup();
    for dir in dirs {
        fs::create_dir_all(dir)?;
    }

    println!("Converting {} file(s)...", jobs.len());

    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    jobs.par_iter().for_each(|job| {
        match convert_dcm_to_png(&job.input, &job.output, no_compression) {
            Ok(()) => {
                println!("  OK: {}", job.output.display());
                success.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("  FAIL: {} — {e}", job.input.display());
                failed.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    println!("\nDone: {s} converted, {f} failed");
    Ok(())
}

fn convert_dcm_to_png(
    input: &Path,
    output: &Path,
    no_compression: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let obj = open_file(input)?;
    let pixel_data = obj.decode_pixel_data()?;
    let image = pixel_data.to_dynamic_image(0)?;

    if no_compression {
        let file = File::create(output)?;
        let writer = BufWriter::new(file);
        let encoder =
            PngEncoder::new_with_quality(writer, CompressionType::Fast, FilterType::NoFilter);
        let img = image.as_bytes();
        encoder.write_image(img, image.width(), image.height(), image.color().into())?;
    } else {
        image.save(output)?;
    }

    Ok(())
}
