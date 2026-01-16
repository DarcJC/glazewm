use std::path::PathBuf;
#[cfg(feature = "build_zebar")]
use std::process::Command;

/// Helper function to run a command via cmd.exe on Windows.
/// This is necessary because pnpm is typically installed as a .cmd script,
/// which cannot be executed directly by Command::new().
#[cfg(feature = "build_zebar")]
fn run_cmd(program: &str, args: &[&str], current_dir: &PathBuf) -> std::io::Result<std::process::ExitStatus> {
    let full_command = format!("{} {}", program, args.join(" "));
    Command::new("cmd")
        .current_dir(current_dir)
        .args(["/C", &full_command])
        .status()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Get the workspace root directory
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let workspace_root = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let zebar_dir = workspace_root.join("thirdparty").join("zebar");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Check if we should build zebar
    #[cfg(feature = "build_zebar")]
    {
        build_zebar(&zebar_dir, &out_dir);
    }

    // If not building, check if prebuilt exists or create a placeholder
    #[cfg(not(feature = "build_zebar"))]
    {
        check_or_create_placeholder(&zebar_dir, &out_dir);
    }
}

#[cfg(feature = "build_zebar")]
fn build_zebar(zebar_dir: &PathBuf, out_dir: &PathBuf) {
    use std::fs;

    println!("cargo:rerun-if-changed={}", zebar_dir.join("packages/desktop/src").display());
    println!("cargo:rerun-if-changed={}", zebar_dir.join("packages/settings-ui/src").display());

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let release_flag = if profile == "release" { "--release" } else { "" };

    // Install pnpm dependencies
    println!("cargo:warning=Installing pnpm dependencies for zebar...");
    let pnpm_install = run_cmd("pnpm", &["install"], zebar_dir);

    if let Err(e) = pnpm_install {
        println!("cargo:warning=Failed to run pnpm install: {}. Make sure pnpm is installed.", e);
        create_placeholder(out_dir);
        return;
    }

    if !pnpm_install.unwrap().success() {
        println!("cargo:warning=pnpm install failed");
        create_placeholder(out_dir);
        return;
    }

    // Build the client-api first (required by settings-ui)
    println!("cargo:warning=Building zebar client-api...");
    let client_api_dir = zebar_dir.join("packages").join("client-api");
    let client_api_build = run_cmd("pnpm", &["run", "build"], &client_api_dir);

    if let Err(e) = client_api_build {
        println!("cargo:warning=Failed to build client-api: {}", e);
        create_placeholder(out_dir);
        return;
    }

    if !client_api_build.unwrap().success() {
        println!("cargo:warning=client-api build failed");
        create_placeholder(out_dir);
        return;
    }

    // Build the settings-ui
    println!("cargo:warning=Building zebar settings-ui...");
    let settings_ui_dir = zebar_dir.join("packages").join("settings-ui");
    let ui_build = run_cmd("pnpm", &["run", "build"], &settings_ui_dir);

    if let Err(e) = ui_build {
        println!("cargo:warning=Failed to build settings-ui: {}", e);
        create_placeholder(out_dir);
        return;
    }

    if !ui_build.unwrap().success() {
        println!("cargo:warning=settings-ui build failed");
        create_placeholder(out_dir);
        return;
    }

    // Build zebar using cargo
    println!("cargo:warning=Building zebar...");
    let mut cargo_args = vec!["build", "-p", "zebar"];
    if !release_flag.is_empty() {
        cargo_args.push(release_flag);
    }

    let cargo_build = Command::new("cargo")
        .current_dir(&zebar_dir)
        .args(&cargo_args)
        .status();

    match cargo_build {
        Ok(status) if status.success() => {
            // Copy the built binary
            let target_dir = zebar_dir.join("target").join(&profile);
            let zebar_exe = target_dir.join("zebar.exe");
            let dest = out_dir.join("zebar.exe");

            if zebar_exe.exists() {
                fs::copy(&zebar_exe, &dest).expect("Failed to copy zebar.exe");
                println!("cargo:warning=Successfully built and copied zebar.exe");
            } else {
                println!("cargo:warning=zebar.exe not found at {:?}", zebar_exe);
                create_placeholder(out_dir);
            }
        }
        Ok(status) => {
            println!("cargo:warning=Cargo build failed with status: {}", status);
            create_placeholder(out_dir);
        }
        Err(e) => {
            println!("cargo:warning=Failed to run cargo build: {}", e);
            create_placeholder(out_dir);
        }
    }
}

#[cfg(not(feature = "build_zebar"))]
fn check_or_create_placeholder(zebar_dir: &PathBuf, out_dir: &PathBuf) {
    use std::fs;

    // Check for prebuilt binary in thirdparty/zebar/prebuilt/
    let prebuilt_path = zebar_dir.join("prebuilt").join("zebar.exe");
    let dest = out_dir.join("zebar.exe");

    if prebuilt_path.exists() {
        fs::copy(&prebuilt_path, &dest).expect("Failed to copy prebuilt zebar.exe");
        println!("cargo:warning=Using prebuilt zebar.exe");
    } else {
        println!("cargo:warning=No prebuilt zebar.exe found. Creating placeholder.");
        println!("cargo:warning=To build zebar from source, enable the 'build_zebar' feature.");
        create_placeholder(out_dir);
    }
}

fn create_placeholder(out_dir: &PathBuf) {
    use std::fs;
    // Create an empty placeholder file
    let dest = out_dir.join("zebar.exe");
    fs::write(&dest, b"").expect("Failed to create placeholder");
}
