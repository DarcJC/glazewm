//! Process management for builtin programs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use crate::{extract_builtin, BuiltinProgram};

/// Global process manager for builtin programs.
static PROCESS_MANAGER: std::sync::OnceLock<Arc<Mutex<ProcessManager>>> =
    std::sync::OnceLock::new();

/// Get the global process manager instance.
pub fn process_manager() -> Arc<Mutex<ProcessManager>> {
    PROCESS_MANAGER
        .get_or_init(|| Arc::new(Mutex::new(ProcessManager::new())))
        .clone()
}

/// Manages running builtin processes.
pub struct ProcessManager {
    /// Map of running processes by program type.
    processes: HashMap<BuiltinProgram, ChildProcess>,
}

struct ChildProcess {
    child: Child,
    #[allow(dead_code)]
    exe_path: PathBuf,
}

impl ProcessManager {
    /// Create a new process manager.
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Start a builtin program.
    pub fn start(&mut self, program: BuiltinProgram) -> Result<()> {
        // Check if already running
        if self.is_running(program) {
            info!("Builtin {:?} is already running", program);
            return Ok(());
        }

        // Extract the binary
        let exe_path = extract_builtin(program)?;

        info!("Starting builtin {:?} from {:?}", program, exe_path);

        // Start the process
        let child = std::process::Command::new(&exe_path)
            .spawn()
            .with_context(|| {
                format!("Failed to start builtin {:?}", program)
            })?;

        info!("Started builtin {:?} with PID {}", program, child.id());

        self.processes.insert(
            program,
            ChildProcess {
                child,
                exe_path,
            },
        );

        Ok(())
    }

    /// Stop a builtin program.
    pub fn stop(&mut self, program: BuiltinProgram) -> Result<()> {
        if let Some(mut process) = self.processes.remove(&program) {
            info!("Stopping builtin {:?} (PID {})", program, process.child.id());

            // Try graceful termination first on Windows
            #[cfg(windows)]
            {
                if let Err(e) = self.terminate_process_tree(process.child.id()) {
                    warn!("Failed to terminate process tree: {}", e);
                    // Fall back to kill
                    if let Err(e) = process.child.kill() {
                        error!("Failed to kill builtin {:?}: {}", program, e);
                    }
                }
            }

            #[cfg(not(windows))]
            {
                if let Err(e) = process.child.kill() {
                    error!("Failed to kill builtin {:?}: {}", program, e);
                }
            }

            // Wait for the process to exit
            let _ = process.child.wait();

            info!("Stopped builtin {:?}", program);
        } else {
            warn!("Builtin {:?} is not running", program);
        }

        Ok(())
    }

    /// Check if a builtin program is running.
    pub fn is_running(&mut self, program: BuiltinProgram) -> bool {
        if let Some(process) = self.processes.get_mut(&program) {
            // Check if the process has exited
            match process.child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited, remove it
                    self.processes.remove(&program);
                    false
                }
                Ok(None) => true,  // Still running
                Err(_) => {
                    // Error checking status, assume not running
                    self.processes.remove(&program);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Stop all running builtin programs.
    pub fn stop_all(&mut self) {
        let programs: Vec<_> = self.processes.keys().copied().collect();
        for program in programs {
            if let Err(e) = self.stop(program) {
                error!("Failed to stop {:?}: {}", program, e);
            }
        }
    }

    /// Terminate a process and all its children on Windows.
    #[cfg(windows)]
    fn terminate_process_tree(&self, pid: u32) -> Result<()> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32First, Process32Next,
            PROCESSENTRY32, TH32CS_SNAPPROCESS,
        };
        use windows::Win32::System::Threading::{
            OpenProcess, TerminateProcess, PROCESS_TERMINATE,
        };

        unsafe {
            // Get all child processes
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;

            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };

            let mut children = Vec::new();

            if Process32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ParentProcessID == pid {
                        children.push(entry.th32ProcessID);
                    }
                    if Process32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);

            // Recursively terminate children
            for child_pid in children {
                let _ = self.terminate_process_tree(child_pid);
            }

            // Terminate the process itself
            if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                let _ = TerminateProcess(handle, 0);
                let _ = CloseHandle(handle);
            }
        }

        Ok(())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Start a builtin program by name.
pub fn start_builtin(name: &str) -> Result<()> {
    let program = BuiltinProgram::from_str(name)
        .with_context(|| format!("Unknown builtin program: {}", name))?;

    process_manager()
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to acquire process manager lock"))?
        .start(program)
}

/// Stop a builtin program by name.
pub fn stop_builtin(name: &str) -> Result<()> {
    let program = BuiltinProgram::from_str(name)
        .with_context(|| format!("Unknown builtin program: {}", name))?;

    process_manager()
        .lock()
        .map_err(|_| anyhow::anyhow!("Failed to acquire process manager lock"))?
        .stop(program)
}

/// Stop all running builtin programs.
pub fn stop_all_builtins() {
    if let Ok(mut manager) = process_manager().lock() {
        manager.stop_all();
    }
}
