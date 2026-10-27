fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let dir = root.join("docs/images");
    let result = bluu_platform::screenshot::write_png(&dir.join("overview.png"), false)
        .and_then(|_| bluu_platform::screenshot::write_png(&dir.join("codecs.png"), true));
    if let Err(e) = result {
        eprintln!("screenshot rendering failed: {e}");
        std::process::exit(1);
    }
    println!("wrote {}/overview.png and codecs.png", dir.display());
}
