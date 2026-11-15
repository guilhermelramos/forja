//! Child processes that never open a window.
//!
//! Forja is a window app. On Windows, a console program (git, tasklist,
//! pwsh…) started by a window app gets a brand-new console window unless it
//! is created with `CREATE_NO_WINDOW`. Every module starts processes through
//! [`command`], so no terminal ever flashes on the person's screen. The
//! integrated terminal uses ConPTY (headless) and is not affected.

use std::ffi::OsStr;
use std::process::Command;

/// `CREATE_NO_WINDOW` process creation flag.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A `std::process::Command` that never opens a console window.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_window(&mut command);
    command
}

/// Marks an existing command so it never opens a console window.
pub fn hide_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    #[test]
    fn hidden_command_still_runs() {
        let output = super::command("cmd").args(["/C", "echo forja"]).output();
        if let Ok(output) = output {
            assert!(String::from_utf8_lossy(&output.stdout).contains("forja"));
        }
    }
}
