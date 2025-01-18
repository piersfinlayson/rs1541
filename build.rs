use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

// Custom error type for our build checks
#[derive(Debug)]
enum BuildError {
    WrapperNotFound(PathBuf),
    PkgConfigMissing,
    LibUsbWrongVersion(String),
    OpenCbmLinkError(String),
    CommandFailed(String, String),
    BindgenError(String),
    IoError(std::io::Error),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::WrapperNotFound(path) => {
                write!(f, "Wrapper header not found at {:?}", path)
            }
            BuildError::PkgConfigMissing => write!(f, "pkg-config not found or failed"),
            BuildError::LibUsbWrongVersion(msg) => write!(f, "Incorrect libusb version: {}", msg),
            BuildError::OpenCbmLinkError(msg) => write!(f, "OpenCBM link error: {}", msg),
            BuildError::CommandFailed(cmd, err) => write!(f, "Command '{}' failed: {}", cmd, err),
            BuildError::BindgenError(msg) => write!(f, "Bindgen error: {}", msg),
            BuildError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<std::io::Error> for BuildError {
    fn from(err: std::io::Error) -> BuildError {
        BuildError::IoError(err)
    }
}

fn check_libusb_version() -> Result<(), BuildError> {
    // Check pkg-config for libusb-1.0
    let output = Command::new("pkg-config")
        .args(["--cflags", "libusb-1.0"])
        .output()
        .map_err(|_| BuildError::PkgConfigMissing)?;

    if !output.status.success() {
        return Err(BuildError::CommandFailed(
            "pkg-config --cflags libusb-1.0".into(),
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }

    let cflags = String::from_utf8_lossy(&output.stdout);
    if !cflags.contains("/libusb-1.0") {
        return Err(BuildError::LibUsbWrongVersion(
            "libusb-1.0 development files not found".into(),
        ));
    }

    Ok(())
}

fn check_opencbm_libusb_linkage() -> Result<(), BuildError> {
    // Check the plugin instead of the main library
    let plugin_path = "/usr/local/lib/opencbm/plugin/libopencbm-xum1541.so";
    
    let output = Command::new("ldd")
        .arg(plugin_path)
        .output()
        .map_err(|_| BuildError::CommandFailed("ldd".into(), 
            format!("Failed to execute ldd on {}", plugin_path)))?;

    if !output.status.success() {
        return Err(BuildError::OpenCbmLinkError(
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }

    let ldd_output = String::from_utf8_lossy(&output.stdout);
    
    // Check for libusb-1.0 presence and absence of old libusb
    if ldd_output.contains("libusb-0.1.so") {
        return Err(BuildError::LibUsbWrongVersion(
            "xum1541 plugin is linked against old libusb-0.1".into(),
        ));
    }

    if !ldd_output.contains("libusb-1.0.so") {
        return Err(BuildError::LibUsbWrongVersion(
            "xum1541 plugin is not linked against libusb-1.0".into(),
        ));
    }

    Ok(())
}

fn generate_bindings(wrapper_path: &Path) -> Result<(), BuildError> {
    // Create the bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header(
            wrapper_path
                .to_str()
                .ok_or_else(|| BuildError::BindgenError("Invalid wrapper path".into()))?,
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .map_err(|_| BuildError::BindgenError("Failed to generate bindings".into()))?;

    // Write the bindings to a file in OUT_DIR
    let out_path = PathBuf::from(
        env::var("OUT_DIR").map_err(|_| BuildError::BindgenError("OUT_DIR not set".into()))?,
    );
    let bindings_path = out_path.join("bindings.rs");

    bindings
        .write_to_file(&bindings_path)
        .map_err(|e| BuildError::BindgenError(format!("Failed to write bindings: {}", e)))?;

    Ok(())
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("Error in build script: {}", e);
        process::exit(1);
    }
}

fn try_main() -> Result<(), BuildError> {
    // Check system dependencies
    check_libusb_version()?;
    check_opencbm_libusb_linkage()?;

    let wrapper_path = Path::new("build/wrapper.h");

    // Ensure wrapper.h exists
    if !wrapper_path.exists() {
        return Err(BuildError::WrapperNotFound(wrapper_path.to_owned()));
    }

    // Link against opencbm
    println!("cargo:rustc-link-lib=opencbm");
    println!("cargo:rerun-if-changed={}", wrapper_path.display());

    // Generate bindings
    generate_bindings(wrapper_path)?;

    // Always rerun if build script changes
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
