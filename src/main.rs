use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::{Parser, ValueEnum};
use dicom_object::open_file;
use dicom_pixeldata::PixelDecoder;
use image::ImageEncoder;
use rayon::prelude::*;

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// PNG (compressed)
    Png,
    /// TIFF (uncompressed)
    Tiff,
}

#[derive(Parser)]
#[command(
    name = "dicom2png",
    about = "Convert DICOM (.dcm) files to PNG or TIFF"
)]
struct Cli {
    /// Input path: a .dcm file, a directory of .dcm files, or a parent directory containing subdirectories of .dcm files
    input: PathBuf,

    /// Output directory (defaults to "output")
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "png")]
    format: OutputFormat,
}

/// A conversion job: input .dcm path and the output path.
struct Job {
    input: PathBuf,
    output: PathBuf,
}

fn file_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Png => "png",
        OutputFormat::Tiff => "tiff",
    }
}

fn collect_jobs(input: &Path, output: &Path, ext: &str) -> Vec<Job> {
    if input.is_file() {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        return vec![Job {
            input: input.to_path_buf(),
            output: output.join(format!("{stem}.{ext}")),
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
                "{}.{ext}",
                path.file_stem().unwrap_or_default().to_string_lossy()
            );
            jobs.push(Job {
                input: path,
                output: output.join(out_name),
            });
        } else if path.is_dir() {
            let folder_name = path.file_name().unwrap_or_default();
            let sub_output = output.join(folder_name);
            jobs.extend(collect_jobs(&path, &sub_output, ext));
        }
    }

    jobs
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let format = cli.format;
    let ext = file_extension(format);

    let jobs = collect_jobs(&cli.input, &cli.output, ext);

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

    println!("Converting {} file(s) to {ext}...", jobs.len());

    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    jobs.par_iter()
        .for_each(|job| match convert_dcm(&job.input, &job.output, format) {
            Ok(()) => {
                println!("  OK: {}", job.output.display());
                success.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("  FAIL: {} — {e}", job.input.display());
                failed.fetch_add(1, Ordering::Relaxed);
            }
        });

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    println!("\nDone: {s} converted, {f} failed");
    Ok(())
}

fn convert_dcm(
    input: &Path,
    output: &Path,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let obj = open_file(input)?;
    let pixel_data = obj.decode_pixel_data()?;
    let image = pixel_data.to_dynamic_image(0)?;

    match format {
        OutputFormat::Png => {
            image.save(output)?;
        }
        OutputFormat::Tiff => {
            let file = File::create(output)?;
            let writer = BufWriter::new(file);
            let encoder = image::codecs::tiff::TiffEncoder::new(writer);
            encoder.write_image(
                image.as_bytes(),
                image.width(),
                image.height(),
                image.color().into(),
            )?;
        }
    }

    Ok(())
}
