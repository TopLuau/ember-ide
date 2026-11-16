fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Ember IDE");
        res.set("FileDescription", "Ember IDE");
        res.set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            // Never fail the build just because the icon could not be embedded.
            println!("cargo:warning=could not embed Windows icon: {e}");
        }
    }
}
