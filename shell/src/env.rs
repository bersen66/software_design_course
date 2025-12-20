use std::collections::HashMap;
use std::env as stdenv;
use std::path::PathBuf;

/// Mutable, user-level view of the process environment used by the interpreter.
///
/// The environment contains:
/// - `vars`: a map of environment variables that will be visible to executed commands.
/// - `current_dir`: the working directory for command execution.
/// - `should_exit`: a flag that a REPL loop can check to know when to terminate.
///
/// Note: fields are public for simplicity to keep the teaching example small.
/// Production code would prefer accessor methods over public fields.
#[derive(Debug, Clone)]
pub struct Environment {
    /// Key-value store of environment variables (e.g., PATH, HOME).
    pub vars: HashMap<String, String>,
    /// The current working directory for command execution.
    pub current_dir: PathBuf,
    /// When set to true, indicates that an interactive loop should exit.
    pub should_exit: bool,
}

impl Environment {
    /// Capture the current process state into a new `Environment` instance.
    ///
    /// This copies variables from `std::env::vars()` and initializes `current_dir`
    /// from `std::env::current_dir()`. The `should_exit` flag is initialized to `false`.
    pub fn new() -> Self {
        let mut vars = HashMap::new();
        for (k, v) in stdenv::vars() {
            vars.insert(k, v);
        }
        let current_dir = stdenv::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            vars,
            current_dir,
            should_exit: false,
        }
    }

    /// Get the value of an environment variable.
    ///
    /// Looks up the key in `self.vars` first, falling back to `std::env::var`.
    pub fn get_var(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .cloned()
            .or_else(|| stdenv::var(key).ok())
    }

    /// Set or override an environment variable in `self.vars`.
    pub fn set_var(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.vars.insert(key.into(), val.into());
    }
}

#[cfg(test)]
mod tests {
    use crate::env::Environment;
    use std::collections::HashMap;
    use std::env as stdenv;

    #[test]
    fn test_env_set_and_get_var() {
        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        // initially absent
        assert_eq!(env.get_var("SOME_RANDOM_ENV_VAR_12345"), None);

        env.set_var("KEY", "VALUE");

        assert_eq!(env.get_var("KEY"), Some("VALUE".to_string()));
    }

    #[test]
    fn test_env_reads_from_process_env() {
        let env = Environment::new();
        assert!(env.get_var("PATH").is_some());
    }
}
