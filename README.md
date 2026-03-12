# dicom2png

A fast CLI tool to convert DICOM (`.dcm`) files to PNG images, written in Rust.

## Features

- Convert single `.dcm` files or entire directories
- Recursive directory traversal with preserved folder structure
- Parallel conversion using Rayon

## Installation

### From source

```bash
cargo install --path .
```

### From GitHub Releases

Download a prebuilt binary for your platform from the
[Releases](https://github.com/sipemu/dicom2png/releases) page.

## Usage

```bash
# Convert all .dcm files in a directory
dicom2png path/to/dicoms/ -o output/

# Convert a parent directory with multiple subdirectories
# Output preserves the folder structure: output/<subfolder>/*.png
dicom2png data/ -o output/

# Convert a single file
dicom2png scan.dcm -o output/
```

### Options

```
Arguments:
  <INPUT>  Input path: a .dcm file, a directory of .dcm files,
           or a parent directory containing subdirectories of .dcm files

Options:
  -o, --output <OUTPUT>  Output directory [default: output]
  -h, --help             Print help
```

## Building

```bash
cargo build --release
```

The binary will be at `target/release/dicom2png`.

## License

MIT
