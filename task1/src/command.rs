use crate::env::Environment;
use anyhow::Result;
use std::io::{Read, Write};

pub type ExitCode = i32;

pub trait ShellCommand {
    fn name(&self) -> &str;

    fn execute(
        &self,
        stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        env: &mut Environment,
    ) -> Result<ExitCode>;
}
