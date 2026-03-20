use dicom_core::Tag;
use dicom_object::open_file;

fn main() {
    for entry in std::fs::read_dir("data/1_B00.12997_135004/").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("dcm") {
            continue;
        }
        let obj = open_file(&path).unwrap();

        let get = |tag: Tag| -> String {
            obj.element(tag)
                .ok()
                .and_then(|e| e.to_str().ok())
                .unwrap_or_default()
                .to_string()
        };

        let rows = get(Tag(0x0028, 0x0010));
        let cols = get(Tag(0x0028, 0x0011));
        let bits = get(Tag(0x0028, 0x0100));
        let stored = get(Tag(0x0028, 0x0101));
        let frames = get(Tag(0x0028, 0x0008));
        let samples = get(Tag(0x0028, 0x0002));
        let pi = get(Tag(0x0028, 0x0004));
        let filesize = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        println!(
            "{}: {}x{}, bits_alloc={}, bits_stored={}, frames={}, samples={}, photo={}, size={}MB",
            path.file_name().unwrap().to_string_lossy(),
            cols,
            rows,
            bits,
            stored,
            if frames.is_empty() {
                "1".into()
            } else {
                frames
            },
            samples,
            pi.trim(),
            filesize / 1_000_000
        );
    }
}
