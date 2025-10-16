use crate::command::{ExitCode, ShellCommand};
use crate::env::Environment;
use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::env;

pub struct Pwd;

impl Pwd {
    pub fn new() -> Self {
        Pwd
    }
}

impl ShellCommand for Pwd {
    fn name(&self) -> &str {
        "pwd"
    }

    fn execute(
        &self,
        _stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        env: &mut Environment,
    ) -> Result<ExitCode> {
        writeln!(stdout, "{}", env.current_dir.to_string_lossy())?;
        Ok(0)
    }
}

pub struct Cd {
    pub target: Option<String>,
}

impl Cd {
    pub fn new(target: Option<String>) -> Self {
        Self { target }
    }
}

impl ShellCommand for Cd {
    fn name(&self) -> &str {
        "cd"
    }

    fn execute(
        &self,
        _stdin: &mut dyn Read,
        _stdout: &mut dyn Write,
        env: &mut Environment,
    ) -> Result<ExitCode> {
        let target = match &self.target {
            Some(t) if !t.is_empty() => PathBuf::from(t),
            _ => {
                if let Some(home) = env.get_var("HOME") {
                    PathBuf::from(home)
                } else {
                    return Err(anyhow::anyhow!("cd: no target and HOME not set"));
                }
            }
        };

        let new_dir = if target.is_absolute() {
            target
        } else {
            env.current_dir.join(target)
        };

        let canonical = fs::canonicalize(&new_dir)
            .with_context(|| format!("cd: can't canonicalize {}", new_dir.display()))?;

        env::set_current_dir(&canonical)
            .with_context(|| format!("cd: can't chdir to {}", canonical.display()))?;
        env.current_dir = canonical;
        Ok(0)
    }
}

pub struct Echo {
    pub args: Vec<String>,
    pub newline: bool,
}

impl Echo {
    pub fn new(args: Vec<String>, newline: bool) -> Self {
        Self { args, newline }
    }
}

impl ShellCommand for Echo {
    fn name(&self) -> &str {
        "echo"
    }

    fn execute(
        &self,
        _stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        let s = self.args.join(" ");
        if self.newline {
            writeln!(stdout, "{}", s)?;
        } else {
            write!(stdout, "{}", s)?;
        }
        Ok(0)
    }
}