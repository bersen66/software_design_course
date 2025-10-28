//! A tiny, embeddable shell-like command runner.
//!
//! This crate provides a minimal set of building blocks to execute built-in commands
//! implemented in Rust and to discover and launch external programs from the current
//! process environment. It is intentionally small and easy to read, suitable for
//! coursework and experiments with process management and argument parsing.
//!
//! The main entry point is [`Interpreter`], which can execute commands by name with
//! arguments using a set of pluggable factories. The public modules [`command`] and
//! [`env`] expose traits and types for implementing your own commands and for
//! interacting with the process environment.

mod builtin;
pub mod command;
pub mod env;
mod external;
mod interpreter;
mod lexer;
mod parser;

/// Just a convenient re-export of the interactive command runner.
///
/// See [`Interpreter`] for the high-level API and examples.
pub use interpreter::Interpreter;
