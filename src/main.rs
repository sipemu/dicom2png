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

/// A conversion job: input .dcm path and the output base path.
struct Job {
    input: PathBuf,
    /// For single-frame: the output file path. For multi-frame: the output directory.
    output_base: PathBuf,
}

fn file_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Png => "png",
        OutputFormat::Tiff => "tiff",
    }
}

fn collect_jobs(input: &Path, output: &Path) -> Vec<Job> {
    if input.is_file() {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        return vec![Job {
            input: input.to_path_buf(),
            output_base: output.join(stem.as_ref()),
        }];
    }

    let mut jobs = Vec::new();

    let Ok(entries) = fs::read_dir(input) else {
        return jobs;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("dcm") {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            jobs.push(Job {
                input: path,
                output_base: output.join(&stem),
            });
        } else if path.is_dir() {
            let folder_name = path.file_name().unwrap_or_default();
            let sub_output = output.join(folder_name);
            jobs.extend(collect_jobs(&path, &sub_output));
        }
    }

    jobs
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let format = cli.format;
    let ext = file_extension(format);

    let jobs = collect_jobs(&cli.input, &cli.output);

    if jobs.is_empty() {
        eprintln!("No .dcm files found in {:?}", cli.input);
        std::process::exit(1);
    }

    fs::create_dir_all(&cli.output)?;

    println!("Converting {} file(s) to {ext}...", jobs.len());

    let success = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    jobs.par_iter().for_each(
        |job| match convert_dcm(&job.input, &job.output_base, format, ext) {
            Ok(count) => {
                println!("  OK: {} ({count} frame(s))", job.input.display());
                success.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("  FAIL: {} — {e}", job.input.display());
                failed.fetch_add(1, Ordering::Relaxed);
            }
        },
    );

    let s = success.load(Ordering::Relaxed);
    let f = failed.load(Ordering::Relaxed);
    println!("\nDone: {s} converted, {f} failed");
    Ok(())
}

fn convert_dcm(
    input: &Path,
    output_base: &Path,
    format: OutputFormat,
    ext: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    let obj = open_file(input)?;
    let pixel_data = obj.decode_pixel_data()?;
    let num_frames = pixel_data.number_of_frames();

    if num_frames <= 1 {
        // Single frame: save directly as output_base.ext
        let output = output_base.with_extension(ext);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let image = pixel_data.to_dynamic_image(0)?;
        save_image(&image, &output, format)?;
    } else {
        // Multi-frame: create a directory and save each frame
        fs::create_dir_all(output_base)?;
        for frame in 0..num_frames {
            let output = output_base.join(format!("{frame:06}.{ext}"));
            let image = pixel_data.to_dynamic_image(frame)?;
            save_image(&image, &output, format)?;
        }
    }

    Ok(num_frames)
}

fn save_image(
    image: &image::DynamicImage,
    output: &Path,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
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
