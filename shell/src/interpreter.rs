use crate::command::{CommandFactory, ExitCode, Stdin};
use crate::env::Environment;
use crate::lexer;
use crate::lexer::WordPart;
use crate::parser::{self, AstNode, Word};
use crate::{MemReader, MemWriter};
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};
use std::io::{Read, Write};
use std::process::Stdio;
use crate::external::find_command_path;
use std::ffi::OsStr;
use std::path::Path;

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
            // if cfg!(debug_assertions) {
            //    println!("Read line: {:?}", readline);
            // }
            match readline {
                Ok(line) => {
                    rl.add_history_entry(line.as_str())?;
                    let tokens = lexer::split_into_tokens(line).unwrap();
                    // if cfg!(debug_assertions) {
                    //     println!("Tokens = {:?}", tokens);
                    // }
                    let ast = parser::construct_ast(tokens).unwrap();
                    // if cfg!(debug_assertions) {
                    //     println!("Ast = {:?}", ast);
                    // }
                    let err = self.execute_ast(&ast);
                    // if err.is_err() {
                    //     println!("Execution error: {:?}", err.err());
                    // }
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

    fn execute_ast_with_redifined_output(&mut self, root: &AstNode, final_stdout: &mut dyn Write,) -> anyhow::Result<ExitCode> {
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
            
            AstNode::Pipeline(commands) => {
                if commands.is_empty() {
                    return Err(anyhow::anyhow!("empty pipeline"));
                }

                let mut previous_output: Option<Vec<u8>> = None;
                let mut last_exit: crate::command::ExitCode = 0;

                for node in commands {
                    let (argv_vec, assignments_ref, _redirects_ref) = match node {
                        AstNode::Command { argv, assignments, redirects } => (argv.clone(), assignments, redirects),
                        _ => return Err(anyhow::anyhow!("pipeline contains non-command node")),
                    };

                    // apply assignments in a local env clone
                    let mut local_env = self.env.clone();
                    for assign in assignments_ref.iter() {
                        if let AstNode::Assignment { name, value } = assign {
                            let val = if let Some(w) = value { self.word_to_string(w)? } else { String::new() };
                            local_env.set_var(name.clone(), val);
                        }
                    }

                    if argv_vec.is_empty() {
                        return Err(anyhow::anyhow!("empty command in pipeline"));
                    }

                    // Resolve name and args with local_env by temporarily swapping self.env
                    let saved_env = std::mem::replace(&mut self.env, local_env.clone());
                    let name = self.word_to_string(&argv_vec[0])?;
                    let args: Vec<String> = argv_vec.iter().skip(1).map(|w| self.word_to_string(w)).collect::<anyhow::Result<Vec<String>>>()?;
                    self.env = saved_env;

                    // Determine if command is external by PATH lookup
                    let is_external = match self.env.get_var("PATH") {
                        Some(paths) => find_command_path(OsStr::new(&paths), Path::new(&name)).is_some(),
                        None => false,
                    };

                    if is_external {
                        // External process: spawn, feed previous_output, read stdout
                        let path = {
                            let p = find_command_path(OsStr::new(&self.env.get_var("PATH").unwrap()), Path::new(&name)).unwrap();
                            p.into_owned()
                        };

                        let mut cmd = std::process::Command::new(path);
                        cmd.args(&args)
                            .envs(self.env.vars.iter().map(|(k,v)| (k.as_str(), v.as_str())))
                            .current_dir(&self.env.current_dir)
                            .stdin(std::process::Stdio::piped())
                            .stdout(std::process::Stdio::piped());

                        let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("failed spawn: {}", e))?;

                        if let Some(buf) = previous_output.take() {
                            if let Some(mut child_stdin) = child.stdin.take() {
                                child_stdin.write_all(&buf).map_err(|e| anyhow::anyhow!(e))?;
                                drop(child_stdin);
                            }
                        } else {
                            drop(child.stdin.take());
                        }

                        let output = child.wait_with_output().map_err(|e| anyhow::anyhow!(e))?;
                        previous_output = Some(output.stdout);
                        last_exit = output.status.code().unwrap_or(1);
                    } else {
                        let mut created: Option<Box<dyn crate::command::ExecutableCommand>> = None;
                        let args_ref_vec: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                        for factory in &self.commands {
                            if let Some(c) = factory.try_create(&self.env, &name, &args_ref_vec) {
                                created = Some(c);
                                break;
                            }
                        }

                        let cmd = created.ok_or_else(|| anyhow::anyhow!("command not found: {}", name))?;

                        let stdin_box: Box<dyn crate::command::Stdin> = if let Some(buf) = previous_output.take() {
                            Box::new(MemReader::new(buf))
                        } else {
                            Box::new(InheritedStdin(std::io::stdin().lock()))
                        };

                        // prepare stdout via with_handle()
                        let (mw, out_rc) = MemWriter::with_handle();
                        let stdout_box: Box<dyn crate::command::Stdout> = Box::new(mw);

                        // execute
                        let mut exec_env = self.env.clone();
                        match cmd.execute(stdin_box, stdout_box, &mut exec_env) {
                            Ok(code) => last_exit = code,
                            Err(_) => last_exit = 1,
                        }

                        previous_output = Some(out_rc.borrow().clone());
                    }
                }

                if let Some(out) = previous_output {
                    final_stdout.write_all(&out)?;
                }
                Ok(last_exit)
            }
            _ => {
                // For now, only handle simple commands
                unimplemented!("Only simple commands are currently supported");
            }
        }
    }

    fn execute_ast(&mut self, root: &AstNode) -> anyhow::Result<ExitCode> {
        self.execute_ast_with_redifined_output(root, &mut std::io::stdout())
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
            Box::new(Factory::<Exit>::default()),
            Box::new(Factory::<ExternalCommand>::default()),
            Box::new(Factory::<Cat>::default()),
            Box::new(Factory::<WC>::default())
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

#[cfg(test)]
mod pipeline_tests {
    use crate::Interpreter;


    #[test]
    fn test_echo_pipe_wc_output() {
        // Prepare interpreter factories: only builtin Echo and Wc and ExternalCommand
        let mut factories: Vec<Box<dyn crate::command::CommandFactory>> = Vec::new();
        factories.push(Box::new(crate::interpreter::Factory::<crate::builtin::Echo>::default()));
        factories.push(Box::new(crate::interpreter::Factory::<crate::builtin::WC>::default()));
        factories.push(Box::new(crate::interpreter::Factory::<crate::external::ExternalCommand>::default()));

        let mut interp = Interpreter::new(factories);

        let line = "echo \"22\" | wc".to_string();
        let tokens = crate::lexer::split_into_tokens(line).unwrap();
        let ast = crate::parser::construct_ast(tokens).unwrap();

        let mut out_buf: Vec<u8> = Vec::new();
        let code = interp.execute_ast_with_redifined_output(&ast, &mut out_buf).unwrap();
        assert_eq!(code, 0);

        let s = String::from_utf8(out_buf).expect("utf8");

        assert_eq!(s, "1 1 3\n");
    }
}
