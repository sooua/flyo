//! Embed the Flyo icon + version metadata into the Windows PE binary.
//!
//! Linux/macOS builds are no-ops — those platforms don't use embedded PE
//! resources; their "icon" is delivered separately (a `.desktop` file
//! pointing at a PNG on Linux, an `.icns` bundle on macOS). For now we only
//! care about Windows, which is where the original webd shipped.
//!
//! `winresource` regenerates the resource only when `assets/flyo.ico` (or
//! Cargo.toml) changes, so incremental builds stay fast.

fn main() {
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

    println!("cargo:rerun-if-changed=assets/flyo.ico");
    println!("cargo:rerun-if-changed=build.rs");
}
