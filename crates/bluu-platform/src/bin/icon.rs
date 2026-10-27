fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let path = root.join("assets/bluu.png");
    match bluu_platform::screenshot::write_icon(&path) {
        Ok(()) => println!("wrote {}", path.display()),
        Err(e) => {
            eprintln!("icon render failed: {e}");
            std::process::exit(1);
        }
    }
}
