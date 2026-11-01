use std::path::Path;

fn main() {
    let target = std::env::var("TARGET").expect("TARGET");
    if target.contains("windows") {
        build_sounds();
    }
}

fn build_sounds() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let sounds_dir = Path::new("assets/sounds");
    let mut entries: Vec<(String, Vec<u8>)> = std::fs::read_dir(sounds_dir)
        .expect("read assets/sounds")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() || path.extension()? != "wav" {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            let raw = std::fs::read(&path).ok()?;
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            std::io::Write::write_all(&mut encoder, &raw).ok()?;
            let compressed = encoder.finish().ok()?;
            Some((name, compressed))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let bin_path = Path::new(&out_dir).join("sounds.bin");
    let rs_path = Path::new(&out_dir).join("sounds_data.rs");

    let mut blob = Vec::new();
    blob.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (name, compressed) in &entries {
        blob.extend_from_slice(&(name.len() as u16).to_le_bytes());
        blob.extend_from_slice(name.as_bytes());
        blob.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        blob.extend_from_slice(compressed);
    }
    std::fs::write(&bin_path, &blob).expect("write sounds.bin");

    std::fs::write(
        &rs_path,
        "pub static SOUND_BYTES: &[u8] = include_bytes!(\"sounds.bin\");\n",
    )
    .expect("write sounds_data.rs");

    println!("cargo:rerun-if-changed=assets/sounds");
}
