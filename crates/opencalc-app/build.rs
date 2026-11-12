//! On Windows, put the application icon and the worksheet icon inside the program,
//! so Explorer and the taskbar show them and a file type can point at them.
//! Without the Windows SDK's resource compiler the build carries on, without icons.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icons/opencalc.ico");
    println!("cargo:rerun-if-changed=../../assets/icons/ocalc.ico");
    #[cfg(windows)]
    embed_icons();
}

#[cfg(windows)]
fn embed_icons() {
    let mut resources = winresource::WindowsResource::new();
    // Resource 1 is the program's own icon; 2 is what a worksheet file shows.
    resources.set_icon("../../assets/icons/opencalc.ico");
    resources.set_icon_with_id("../../assets/icons/ocalc.ico", "2");
    resources.set("FileDescription", "OpenCalc");
    resources.set("ProductName", "OpenCalc");
    if let Err(error) = resources.compile() {
        println!("cargo:warning=the icons were not embedded: {error}");
    }
}
