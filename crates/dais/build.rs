fn main() {
    // Embed application icon into the Windows executable.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        // Use the project icon (PNG converted to ICO at build time isn't
        // supported by winresource directly, so we point to an .ico file).
        // If assets/dais.ico exists, embed it; otherwise skip gracefully.
        let ico_path = std::path::Path::new("../../assets/dais.ico");
        if ico_path.exists() {
            res.set_icon(ico_path.to_str().unwrap());
            if let Err(e) = res.compile() {
                eprintln!("cargo:warning=Failed to embed icon: {e}");
            }
        } else {
            eprintln!("cargo:warning=assets/dais.ico not found — skipping icon embedding");
        }
    }
}
