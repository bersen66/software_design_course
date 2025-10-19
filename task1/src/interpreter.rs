use crate::command::{CommandFactory, ExitCode, Stdin};
use crate::env::Environment;
use std::io::Read;
use std::process::Stdio;

/// Factory allows creating instances of ExecutableCommand.
///
/// Only support commands defined in this crate — BuiltinCommand and ExternalCommand.
pub(crate) struct Factory<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for Factory<T> {
    fn default() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

/// A minimal shell-like interpreter that can execute built-in and external commands.
///
/// The interpreter maintains an [`Environment`] and a list of [`CommandFactory`] objects
/// that are queried to create commands by name. See [`Default`] for the built-in
/// factories included out of the box.
///
/// Example
/// ```
/// use shell_commands::Interpreter;
/// let mut sh = Interpreter::default();
/// let code = sh.run("echo", &["hello", "world"]).unwrap();
/// assert_eq!(code, 0);
/// ```
pub struct Interpreter {
    env: Environment,
    commands: Vec<Box<dyn CommandFactory>>,
}

impl Interpreter {
    /// Create a new interpreter with a custom set of command factories.
    pub fn new(commands: Vec<Box<dyn CommandFactory>>) -> Self {
        Self {
            env: Environment::new(),
            commands,
        }
    }

    /// Run a single command invocation by name with arguments.
    ///
    /// Returns the command's exit code or an error if the command cannot be created
    /// or fails to execute.
    pub fn run(&mut self, name: &str, args: &[&str]) -> anyhow::Result<ExitCode> {
        let stdin = InheritedStdin(std::io::stdin().lock());
        for factory in &self.commands {
            if let Some(cmd) = factory.try_create(&self.env, name, args) {
                return cmd.execute(Box::new(stdin), Box::new(std::io::stdout()), &mut self.env);
            }
        }
        Err(anyhow::anyhow!("command not found: {}", name))
    }

    /// A placeholder Read-Eval-Print Loop implementation.
    pub fn repl(&mut self) {
        while !self.env.should_exit {
            let _parsed = todo!();
        }
    }
}

impl Default for Interpreter {
    /// Create an interpreter with the default set of commands:
    /// - built-ins: `pwd`, `cd`, `echo`
    /// - external command launcher
    fn default() -> Self {
        use crate::builtin::*;
        use crate::external::ExternalCommand;
        Self::new(vec![
            Box::new(Factory::<Pwd>::default()),
            Box::new(Factory::<Cd>::default()),
            Box::new(Factory::<Echo>::default()),
            Box::new(Factory::<ExternalCommand>::default()),
        ])
    }
}

struct InheritedStdin<'a>(std::io::StdinLock<'a>);

impl Read for InheritedStdin<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Stdin for InheritedStdin<'_> {
    fn stdio(self: Box<Self>) -> Stdio {
        Stdio::inherit()
    }
}
