//! Embedded binary data and extraction utilities.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::info;

/// Embedded zebar binary data.
/// This will be an empty file if zebar was not built.
const ZEBAR_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/zebar.exe"));

/// List of available builtin programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinProgram {
    Zebar,
}

impl BuiltinProgram {
    /// Parse a builtin program name from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "zebar" => Some(Self::Zebar),
            _ => None,
        }
    }

    /// Get the executable name for this builtin program.
    pub fn exe_name(&self) -> &'static str {
        match self {
            Self::Zebar => "zebar.exe",
        }
    }

    /// Get the embedded binary data for this program.
    pub fn binary_data(&self) -> &'static [u8] {
        match self {
            Self::Zebar => ZEBAR_BINARY,
        }
    }

    /// Check if this builtin program is available (was actually embedded).
    pub fn is_available(&self) -> bool {
        !self.binary_data().is_empty()
    }
}

/// Get the directory where builtin binaries are extracted.
pub fn get_builtin_dir() -> Result<PathBuf> {
    let home = home::home_dir().context("Unable to get home directory")?;
    let builtin_dir = home.join(".glzr").join("glazewm").join("builtin");

    if !builtin_dir.exists() {
        fs::create_dir_all(&builtin_dir)
            .context("Failed to create builtin directory")?;
    }

    Ok(builtin_dir)
}

/// Extract a builtin program to disk if needed.
/// Returns the path to the extracted executable.
pub fn extract_builtin(program: BuiltinProgram) -> Result<PathBuf> {
    if !program.is_available() {
        anyhow::bail!(
            "Builtin program {:?} is not available. \
            It may not have been built. Enable the 'build_zebar' feature \
            or provide a prebuilt binary.",
            program
        );
    }

    let builtin_dir = get_builtin_dir()?;
    let exe_path = builtin_dir.join(program.exe_name());

    // Check if we need to extract (file doesn't exist or is different)
    let needs_extraction = if exe_path.exists() {
        // Compare file sizes first (quick check)
        let existing_size = fs::metadata(&exe_path)
            .map(|m| m.len())
            .unwrap_or(0);
        existing_size != program.binary_data().len() as u64
    } else {
        true
    };

    if needs_extraction {
        info!("Extracting builtin {:?} to {:?}", program, exe_path);

        let mut file = fs::File::create(&exe_path)
            .context("Failed to create builtin executable file")?;

        file.write_all(program.binary_data())
            .context("Failed to write builtin executable data")?;

        file.flush()?;

        info!("Successfully extracted builtin {:?}", program);
    }

    Ok(exe_path)
}

/// Get all available builtin programs.
pub fn available_builtins() -> Vec<BuiltinProgram> {
    let all = [BuiltinProgram::Zebar];
    all.into_iter().filter(|p| p.is_available()).collect()
}
