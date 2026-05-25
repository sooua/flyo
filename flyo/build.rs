//! Embed the Flyo icon + version metadata into the Windows PE binary.
//!
//! Linux / macOS hosts have no `winresource` crate available (it's declared
//! only under `[target.'cfg(windows)'.build-dependencies]`). Guarding both
//! the import and the call site with `#[cfg(windows)]` keeps build.rs
//! compiling on every host even when the workspace is built natively on
//! Linux for Linux targets.
//!
//! Note: this skips the icon when cross-compiling FROM Linux TO Windows.
//! Our CI matrix builds Windows targets on Windows runners, so this is a
//! non-issue in practice.

fn main() {
    #[cfg(windows)]
    embed_windows_resources();

    println!("cargo:rerun-if-changed=assets/flyo.ico");
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(windows)]
fn embed_windows_resources() {
    // Belt-and-braces: even on a Windows host, only embed when the target is
    // also Windows. Cross-compiling Windows host → Linux target should ship
    // a clean ELF without PE resource sections.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/flyo.ico")
        .set("ProductName", "Flyo")
        .set("FileDescription", "Flyo — lightweight self-hosted file server")
        .set("CompanyName", "Flyo")
        .set("LegalCopyright", "MIT licensed");
    if let Err(e) = res.compile() {
        // Don't fail the build on machines without rc.exe / windres — just
        // warn so developers without the toolchain can still iterate.
        println!("cargo:warning=icon embed skipped: {e}");
    }
}
