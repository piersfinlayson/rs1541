use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    if let Err(e) = try_main() {
        eprintln!("Error in build script: {}", e);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let wrapper_path = Path::new("build/wrapper.h");
    
    // Ensure wrapper.h exists
    if !wrapper_path.exists() {
        return Err(format!("Cannot find wrapper header at {:?}", wrapper_path).into());
    }

    // Link against opencbm
    println!("cargo:rustc-link-lib=opencbm");
    println!("cargo:rerun-if-changed={}", wrapper_path.display());

    // Create the bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header(wrapper_path.to_str().ok_or("Invalid wrapper path")?)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .map_err(|_| "Unable to generate bindings")?;

    // Write the bindings to a file in OUT_DIR
    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    let bindings_path = out_path.join("bindings.rs");
    
    bindings
        .write_to_file(&bindings_path)
        .map_err(|e| format!("Couldn't write bindings to {:?}: {}", bindings_path, e))?;

    // Build scripts should only emit cargo: messages, not general output
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}