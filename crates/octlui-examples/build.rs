use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=src/features");
    let directory = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("src/features");
    let mut files = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "rs") && path.is_file());
    files.sort();

    let mut modules = String::new();
    let mut registry = String::from("const FEATURES: &[Feature] = &[\n");
    for (i, path) in files.iter().enumerate() {
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| io::Error::other("feature filenames must be UTF-8"))?;
        let path = path
            .to_str()
            .ok_or_else(|| io::Error::other("feature paths must be UTF-8"))?;
        modules.push_str(&format!("#[path = {path:?}] mod page_{i};\n"));
        registry.push_str(&format!(
            "Feature {{ id: {id:?}, category: page_{i}::CATEGORY, title: page_{i}::TITLE, view: page_{i}::view }},\n"
        ));
    }
    registry.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("features.rs");
    fs::write(output, modules + &registry)
}
