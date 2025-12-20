use crate::env::Environment;
use anyhow::Result;
use std::io::{Read, Write};
use std::process::Stdio;

/// Conventional process exit code type used by this crate.
///
/// A value of 0 indicates success; any non-zero value indicates failure.
/// This mirrors the convention used by POSIX shells and many command-line tools.
pub type ExitCode = i32;

/// Abstraction over a readable input stream that can also be converted into
/// a [`Stdio`] handle for spawning external processes.
///
/// Implementors typically wrap standard input or a pipe. A blanket implementation
/// exists for any type that implements `Read` and `Into<Stdio>` (e.g. `Sdout` or `Stderr`).
pub trait Stdin: Read {
    /// Convert this input into a [`Stdio`] handle suitable for `std::process::Command`.
    fn stdio(self: Box<Self>) -> Stdio;
}

impl<T: Read + Into<Stdio>> Stdin for T {
    fn stdio(self: Box<Self>) -> Stdio {
        (*self).into()
    }
}

/// Abstraction over a writable output stream that can also be converted into
/// a [`Stdio`] handle for spawning external processes.
///
/// A blanket implementation exists for any type that implements `Write` and `Into<Stdio>`.
pub trait Stdout: Write {
    /// Convert this output into a [`Stdio`] handle suitable for `std::process::Command`.
    fn stdio(self: Box<Self>) -> Stdio;
}

impl<T: Write + Into<Stdio>> Stdout for T {
    fn stdio(self: Box<Self>) -> Stdio {
        (*self).into()
    }
}

/// Object-safe trait for any command that can be executed by the shell.
///
/// This is implemented by built-ins via a blanket impl and by external commands.
pub trait ExecutableCommand {
    /// Executes the command.
    fn execute(
        self: Box<Self>,
        stdin: Box<dyn Stdin>,
        stdout: Box<dyn Stdout>,
        env: &mut Environment,
    ) -> Result<ExitCode>;
}

/// Factory that tries to create a command from a name and its arguments.
///
/// Returns `None` when the factory doesn't recognize the `name`.
/// Implementations can use the environment to resolve executables (e.g., using PATH).
pub trait CommandFactory {
    /// Attempt to create a command instance for the provided name and arguments.
    fn try_create(
        &self,
        env: &Environment,
        name: &str,
        args: &[&str],
    ) -> Option<Box<dyn ExecutableCommand>>;
}
