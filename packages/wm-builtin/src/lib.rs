//! Builtin embedded binaries management for GlazeWM.
//!
//! This module provides functionality to embed and manage builtin binaries
//! (like zebar) within the GlazeWM executable.

mod embedded;
mod process_manager;

pub use embedded::*;
pub use process_manager::*;
