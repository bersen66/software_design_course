use crate::command::{CommandFactory, ExitCode, Stdin};
use crate::env::Environment;
use crate::lexer;
use crate::lexer::WordPart;
use crate::parser::{self, AstNode, Word};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use std::fs::read;
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
    pub fn repl(&mut self) -> Result<()> {
        // Stolen from basic example in: https://github.com/kkawakam/rustyline
        let mut rl = DefaultEditor::new()?;

        loop {
            // Added monke
            let readline = rl.readline("🐒$ ");
            println!("Readed line: {:?}", readline);
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str())?;
                    let tokens = lexer::split_into_tokens(line).unwrap();
                    println!("Tokens = {:?}", tokens);
                    let ast = parser::construct_ast(tokens).unwrap();
                    println!("Ast = {:?}", ast);
                    let err = self.execute_ast(&ast);
                    if err.is_err() {
                        println!("Execution error: {:?}", err.err());
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("Interrupted");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("Eof");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }

        Ok(())
    }

    fn execute_ast(&mut self, root: &AstNode) -> anyhow::Result<ExitCode> {
        match root {
            AstNode::Command {
                argv,
                assignments,
                redirects: _,
            } => {
                // First handle variable assignments
                for assignment in assignments {
                    if let AstNode::Assignment { name, value } = assignment {
                        let value_str = if let Some(word) = value {
                            self.word_to_string(word)?
                        } else {
                            String::new() // Empty string for assignments like VAR=
                        };
                        self.env.set_var(name, value_str);
                    }
                }

                if argv.is_empty() {
                    return Ok(0); // Empty command, just return success
                }

                // Convert the first word to command name
                let name = self.word_to_string(&argv[0])?;

                // Convert arguments from Word to String with environment substitution
                let args: Vec<String> = argv
                    .iter()
                    .skip(1)
                    .map(|word| self.word_to_string(word))
                    .collect::<anyhow::Result<Vec<String>>>()?;

                // Convert Vec<String> to Vec<&str> for the run method
                let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

                self.run(&name, &args_ref)
            }
            _ => {
                // For now, only handle simple commands
                unimplemented!("Only simple commands are currently supported");
            }
        }
    }

    /// Helper method to convert a Word to a String with environment variable substitution
    fn word_to_string(&self, word: &Word) -> anyhow::Result<String> {
        match word {
            Word::Literal(s) => Ok(s.clone()),
            Word::Compound(parts) => {
                // For compound words, concatenate all parts, substituting parameters as we go
                let mut result = String::new();
                for part in parts {
                    match part {
                        WordPart::Literal(text) => result.push_str(text),
                        WordPart::ParamSubst(var_name) => {
                            // Handle parameter substitution ${VAR} or $VAR
                            if let Some(value) = self.env.get_var(var_name) {
                                result.push_str(&value);
                            }
                            // If variable doesn't exist, substitute with empty string (like bash)
                        }
                        WordPart::CmdSubst(_) => {
                            return Err(anyhow::anyhow!("Command substitutions not yet supported"));
                        }
                    }
                }
                Ok(result)
            }
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
