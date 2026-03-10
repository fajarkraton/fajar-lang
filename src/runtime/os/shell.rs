//! Mini kernel shell — processes basic commands.
//!
//! Provides a simple command processor for the kernel demo:
//! `help`, `clear`, `echo`, `ticks`, `uptime`.

// ═══════════════════════════════════════════════════════════════════════
// Shell command result
// ═══════════════════════════════════════════════════════════════════════

/// Result of executing a shell command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellResult {
    /// Text output to display.
    Output(String),
    /// Request to clear the screen.
    Clear,
    /// Unknown command.
    Unknown(String),
    /// Empty input (no-op).
    Empty,
}

// ═══════════════════════════════════════════════════════════════════════
// Mini shell
// ═══════════════════════════════════════════════════════════════════════

/// Minimal kernel shell with built-in commands.
#[derive(Debug)]
pub struct MiniShell {
    /// Shell prompt string.
    prompt: String,
    /// Command history.
    history: Vec<String>,
}

impl MiniShell {
    /// Create a new shell with the given prompt.
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            history: Vec::new(),
        }
    }

    /// Get the prompt string.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Execute a command line and return the result.
    pub fn execute(&mut self, line: &str) -> ShellResult {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return ShellResult::Empty;
        }

        self.history.push(trimmed.to_string());

        let mut parts = trimmed.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let args = parts.next().unwrap_or("").trim();

        match cmd {
            "help" => ShellResult::Output(self.cmd_help()),
            "clear" | "cls" => ShellResult::Clear,
            "echo" => ShellResult::Output(args.to_string()),
            "history" => ShellResult::Output(self.cmd_history()),
            "version" => ShellResult::Output("Fajar Lang Kernel v0.3.0".to_string()),
            "uname" => ShellResult::Output("FajarOS x86_64".to_string()),
            _ => ShellResult::Unknown(cmd.to_string()),
        }
    }

    /// Get command history.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Help text.
    fn cmd_help(&self) -> String {
        "Available commands:\n  help     - Show this help\n  clear    - Clear screen\n  echo     - Echo text\n  history  - Show command history\n  version  - Show kernel version\n  uname    - Show system info".to_string()
    }

    /// History output.
    fn cmd_history(&self) -> String {
        self.history
            .iter()
            .enumerate()
            .map(|(i, cmd)| format!("  {}: {}", i + 1, cmd))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for MiniShell {
    fn default() -> Self {
        Self::new("fj> ")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_help() {
        let mut shell = MiniShell::default();
        let result = shell.execute("help");
        match result {
            ShellResult::Output(text) => {
                assert!(text.contains("help"));
                assert!(text.contains("clear"));
                assert!(text.contains("echo"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn shell_echo() {
        let mut shell = MiniShell::default();
        assert_eq!(
            shell.execute("echo Hello World"),
            ShellResult::Output("Hello World".to_string())
        );
    }

    #[test]
    fn shell_clear() {
        let mut shell = MiniShell::default();
        assert_eq!(shell.execute("clear"), ShellResult::Clear);
        assert_eq!(shell.execute("cls"), ShellResult::Clear);
    }

    #[test]
    fn shell_unknown_command() {
        let mut shell = MiniShell::default();
        assert_eq!(
            shell.execute("foobar"),
            ShellResult::Unknown("foobar".to_string())
        );
    }

    #[test]
    fn shell_empty_input() {
        let mut shell = MiniShell::default();
        assert_eq!(shell.execute(""), ShellResult::Empty);
        assert_eq!(shell.execute("   "), ShellResult::Empty);
    }

    #[test]
    fn shell_history() {
        let mut shell = MiniShell::default();
        shell.execute("help");
        shell.execute("echo test");
        assert_eq!(shell.history().len(), 2);

        let result = shell.execute("history");
        match result {
            ShellResult::Output(text) => {
                assert!(text.contains("help"));
                assert!(text.contains("echo test"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn shell_version() {
        let mut shell = MiniShell::default();
        assert_eq!(
            shell.execute("version"),
            ShellResult::Output("Fajar Lang Kernel v0.3.0".to_string())
        );
    }

    #[test]
    fn shell_prompt() {
        let shell = MiniShell::new("kern$ ");
        assert_eq!(shell.prompt(), "kern$ ");
    }
}
