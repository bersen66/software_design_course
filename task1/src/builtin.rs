use crate::command::{CommandFactory, ExecutableCommand, ExitCode, Stdin, Stdout};
use crate::env::Environment;
use crate::interpreter::Factory;
use anyhow::{Context, Result};
use argh::{EarlyExit, FromArgs};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Built-in commands known to the shell at compile time.
///
/// Builtins are parsed using the [`argh`] crate (`FromArgs`) and executed directly
/// in-process without spawning a child process.
pub(crate) trait BuiltinCommand: Sized + FromArgs {
    /// Canonical name of the command, e.g. "echo" or "cd".
    fn name() -> &'static str;

    /// Executes the command using provided IO streams and environment.
    ///
    /// Return value should follow shell conventions: 0 for success, non-zero for error.
    fn execute(
        self,
        stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        env: &mut Environment,
    ) -> Result<ExitCode>;
}

impl<T: BuiltinCommand> ExecutableCommand for T {
    fn execute(
        self: Box<Self>,
        mut stdin: Box<dyn Stdin>,
        mut stdout: Box<dyn Stdout>,
        env: &mut Environment,
    ) -> Result<ExitCode> {
        match T::execute(*self, &mut stdin, &mut stdout, env) {
            Ok(x) => Ok(x),
            Err(e) => {
                stdout.write_all(e.to_string().as_bytes())?;
                Ok(1)
            }
        }
    }
}

struct InvalidArgs {
    output: String,
    is_error: bool,
}

impl ExecutableCommand for InvalidArgs {
    fn execute(
        self: Box<Self>,
        _stdin: Box<dyn Stdin>,
        mut stdout: Box<dyn Stdout>,
        _env: &mut Environment,
    ) -> anyhow::Result<i32> {
        stdout.write_all(self.output.as_bytes())?;
        Ok(if self.is_error { 1 } else { 0 })
    }
}

impl<T: BuiltinCommand + 'static> CommandFactory for Factory<T> {
    fn try_create(
        &self,
        _env: &Environment,
        name: &str,
        args: &[&str],
    ) -> Option<Box<dyn ExecutableCommand>> {
        if name == T::name() {
            Some(match T::from_args(&[name], args) {
                Ok(cmd) => Box::new(cmd),
                Err(EarlyExit { output, status }) => Box::new(InvalidArgs {
                    output,
                    is_error: status.is_err(),
                }),
            })
        } else {
            None
        }
    }
}

#[derive(FromArgs)]
/// Print the current working directory to standard output.
pub struct Pwd {}

impl BuiltinCommand for Pwd {
    fn name() -> &'static str {
        "pwd"
    }

    fn execute(
        self,
        _stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        env: &mut Environment,
    ) -> Result<ExitCode> {
        writeln!(stdout, "{}", env.current_dir.to_string_lossy())?;
        Ok(0)
    }
}

#[derive(FromArgs)]
/// Change the current working directory.
/// If no target is provided, changes to the directory specified by the HOME environment variable.
pub struct Cd {
    #[argh(positional)]
    /// directory to switch to; absolute or relative to the current directory. Defaults to $HOME when omitted.
    pub target: Option<String>,
}

impl BuiltinCommand for Cd {
    fn name() -> &'static str {
        "cd"
    }

    fn execute(
        self,
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

#[derive(FromArgs)]
/// write the arguments to standard output, separated by spaces.
/// by default, a trailing newline is printed.
pub struct Echo {
    #[argh(switch, short = 'n')]
    /// do not output the trailing newline.
    pub no_newline: bool,

    #[argh(positional, greedy)]
    /// values to print as-is, separated by spaces.
    pub args: Vec<String>,
}

impl BuiltinCommand for Echo {
    fn name() -> &'static str {
        "echo"
    }

    fn execute(
        self,
        _stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        let s = self.args.join(" ");
        if self.no_newline {
            write!(stdout, "{}", s)?;
        } else {
            writeln!(stdout, "{}", s)?;
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::env as stdenv;
    use std::io;
    use std::io::Cursor;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn lock_current_dir() -> MutexGuard<'static, ()> {
        static MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn test_pwd_prints_current_dir() {
        let _lock = lock_current_dir();
        let cur = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: cur.clone(),
            should_exit: false,
        };

        let mut out = Vec::new();
        let cmd = Pwd {};
        let res = cmd.execute(&mut Cursor::new(Vec::new()), &mut out, &mut env);

        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        let expected = format!("{}\n", cur.to_string_lossy());

        assert_eq!(s, expected);
    }

    #[test]
    fn test_echo_with_and_without_newline() {
        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        // With newline
        let mut out1 = Vec::new();
        let args = vec!["hello".to_string(), "world".to_string()];
        let echo1 = Echo {
            no_newline: false,
            args,
        };
        let res1 = echo1.execute(&mut Cursor::new(Vec::new()), &mut out1, &mut env);

        assert!(res1.is_ok());
        assert_eq!(String::from_utf8(out1).unwrap(), "hello world\n");

        // Without newline
        let mut out2 = Vec::new();
        let args = vec!["foo".to_string(), "bar".to_string()];
        let echo2 = Echo {
            no_newline: true,
            args,
        };
        let res2 = echo2.execute(&mut Cursor::new(Vec::new()), &mut out2, &mut env);

        assert!(res2.is_ok());
        assert_eq!(String::from_utf8(out2).unwrap(), "foo bar");
    }

    fn make_unique_temp_dir() -> io::Result<PathBuf> {
        let mut p = stdenv::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("task1_test_cd_{}_{}", std::process::id(), nanos));
        fs::create_dir_all(&p)?;
        Ok(p)
    }

    #[test]
    fn test_cd_to_absolute_path() {
        let _lock = lock_current_dir();
        let temp = make_unique_temp_dir().expect("failed to create temp dir");
        let canonical_temp = fs::canonicalize(&temp).expect("canonicalize failed");

        // save original cwd to restore later
        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        let target = Some(canonical_temp.to_string_lossy().to_string());
        let cmd = Cd { target };
        let res = cmd.execute(&mut Cursor::new(Vec::new()), &mut Vec::new(), &mut env);

        assert!(res.is_ok());

        let new_cwd = stdenv::current_dir().unwrap();
        let new_canonical = fs::canonicalize(&new_cwd).unwrap();

        assert_eq!(new_canonical, canonical_temp);
        assert_eq!(env.current_dir, canonical_temp);

        stdenv::set_current_dir(orig).expect("failed to restore cwd");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_cd_to_home_when_none() {
        let _lock = lock_current_dir();
        let temp = make_unique_temp_dir().expect("failed to create temp dir");
        let canonical_temp = fs::canonicalize(&temp).expect("canonicalize failed");

        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        env.set_var("HOME", canonical_temp.to_string_lossy().to_string());

        let cmd = Cd { target: None };
        let res = cmd.execute(&mut Cursor::new(Vec::new()), &mut Vec::new(), &mut env);

        assert!(res.is_ok());

        let new_cwd = stdenv::current_dir().unwrap();
        let new_canonical = fs::canonicalize(&new_cwd).unwrap();

        assert_eq!(new_canonical, canonical_temp);
        assert_eq!(env.current_dir, canonical_temp);

        stdenv::set_current_dir(orig).expect("failed to restore cwd");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_cd_nonexistent_path_errors() {
        let _lock = lock_current_dir();
        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        let name = format!("nonexistent_dir_for_task1_test_{}", std::process::id());
        let target = Some(name);
        let cmd = Cd { target };
        let res = cmd.execute(&mut Cursor::new(Vec::new()), &mut Vec::new(), &mut env);

        assert!(res.is_err());
        assert_eq!(stdenv::current_dir().unwrap(), orig);
    }
}
