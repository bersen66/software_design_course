use crate::command::{CommandFactory, ExecutableCommand, ExitCode, Stdin, Stdout};
use crate::env::Environment;
use crate::interpreter::Factory;
use anyhow::{Context, Result};
use argh::{EarlyExit, FromArgs};
use regex::RegexBuilder;
use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
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
/// Exit shell process
pub struct Exit {
    #[argh(positional, greedy)]
    /// hack to ignore arguments, but actually should accept 0-255 value
    pub _args: Vec<String>,
}

impl BuiltinCommand for Exit {
    fn name() -> &'static str {
        "exit"
    }

    fn execute(
        self,
        _stdin: &mut dyn Read,
        _stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        std::process::exit(0)
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

#[derive(argh::FromArgs)]
/// count lines, words and bytes
pub struct WC {
    #[argh(positional, greedy)]
    pub files: Vec<String>,
}

impl BuiltinCommand for WC {
    fn name() -> &'static str {
        "wc"
    }

    fn execute(
        self,
        stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        use std::io::Read;
        if self.files.is_empty() {
            let mut buf = String::new();
            stdin.read_to_string(&mut buf)?;
            let lines = buf.lines().count();
            let words = buf.split_whitespace().count();
            let bytes = buf.as_bytes().len();
            writeln!(stdout, "{} {} {}", lines, words, bytes)?;
            return Ok(0);
        }
        for fname in self.files {
            let mut f =
                std::fs::File::open(&fname).map_err(|e| anyhow::anyhow!("wc: {}: {}", fname, e))?;
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            let lines = s.lines().count();
            let words = s.split_whitespace().count();
            let bytes = s.as_bytes().len();
            writeln!(stdout, "{} {} {} {}", lines, words, bytes, fname)?;
        }
        Ok(0)
    }
}

#[derive(argh::FromArgs)]
/// print file(s) to stdout
pub struct Cat {
    #[argh(positional, greedy)]
    pub files: Vec<String>,
}

impl BuiltinCommand for Cat {
    fn name() -> &'static str {
        "cat"
    }

    fn execute(
        self,
        _stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        if self.files.is_empty() {
            // read stdin to stdout
            let mut buf = String::new();
            _stdin.read_to_string(&mut buf)?;
            write!(stdout, "{}", buf)?;
            return Ok(0);
        }
        for fname in self.files {
            let mut f = std::fs::File::open(&fname)
                .map_err(|e| anyhow::anyhow!("cat: {}: {}", fname, e))?;
            std::io::copy(&mut f, stdout)?;
        }
        Ok(0)
    }
}

#[derive(argh::FromArgs)]
/// print lines matching a pattern
pub struct Grep {
    #[argh(positional)]
    /// the pattern to search for (a regular expression)
    pub pattern: String,

    #[argh(positional, greedy)]
    /// files to search. If none provided, reads from stdin.
    pub files: Vec<String>,

    #[argh(switch, short = 'w')]
    /// match only whole words (using non-word characters as boundaries)
    pub word_regexp: bool,

    #[argh(switch, short = 'i')]
    /// ignore case distinctions
    pub ignore_case: bool,

    #[argh(option, short = 'A', default = "0")]
    /// print NUM lines of trailing context after matching lines
    pub after_context: usize,
}

impl Grep {
    fn process_source(
        &self,
        reader: &mut dyn Read,
        stdout: &mut dyn Write,
        file_name: Option<&str>,
        re: &regex::Regex,
    ) -> Result<ExitCode> {
        let mut reader = BufReader::new(reader);

        let mut lines = Vec::new();
        let mut match_indices = Vec::new();

        let mut line_num = 0;
        loop {
            let mut line = String::new();

            let bytes_read = match reader.read_line(&mut line) {
                Ok(n) => n,
                Err(e) => return Err(anyhow::anyhow!("read error: {}", e)),
            };

            if bytes_read == 0 {
                break;
            }

            if re.is_match(&line) {
                match_indices.push(line_num);
            }

            lines.push(line);
            line_num += 1;
        }

        if lines.is_empty() {
            return Ok(0);
        }

        let total_lines = lines.len();
        let mut to_print = vec![false; total_lines];
        let context_end = self.after_context;

        for &match_line in &match_indices {
            let start = match_line;
            let end_print = (match_line + context_end + 1).min(total_lines);

            for i in start..end_print {
                to_print[i] = true;
            }
        }

        let prefix = file_name
            .map(|name| format!("{}:", name))
            .unwrap_or_default();
        let mut last_printed_index: Option<usize> = None;
        let separator = if self.after_context > 0 { "--\n" } else { "" };

        for (i, line) in lines.iter().enumerate() {
            if to_print[i] {
                if self.after_context > 0
                    && last_printed_index.is_some()
                    && i > last_printed_index.unwrap() + 1
                {
                    stdout.write_all(separator.as_bytes())?;
                }

                write!(stdout, "{}{}", prefix, line)?;
                last_printed_index = Some(i);
            }
        }

        Ok(0)
    }
}

impl BuiltinCommand for Grep {
    fn name() -> &'static str {
        "grep"
    }

    fn execute(
        self,
        stdin: &mut dyn Read,
        stdout: &mut dyn Write,
        _env: &mut Environment,
    ) -> Result<ExitCode> {
        let user_pattern = self.pattern.clone();

        let pattern = if self.word_regexp {
            format!(r"\b({})\b", user_pattern)
        } else {
            user_pattern
        };

        let re = RegexBuilder::new(&pattern)
            .case_insensitive(self.ignore_case)
            .build()
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        if self.files.is_empty() {
            // Чтение из stdin
            self.process_source(stdin, stdout, None, &re)
        } else {
            // Чтение из файлов
            let mut final_exit_code = 0;
            for file_name in &self.files {
                match fs::File::open(file_name) {
                    Ok(mut f) => {
                        if let Err(e) = self.process_source(&mut f, stdout, Some(file_name), &re) {
                            writeln!(stdout, "grep: {}: {}", file_name, e)?;
                            final_exit_code = 1;
                        }
                    }
                    Err(e) => {
                        writeln!(stdout, "grep: {}: {}", file_name, e)?;
                        final_exit_code = 1;
                    }
                }
            }
            Ok(final_exit_code)
        }
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

    #[test]
    fn test_cat_reads_file() {
        let _lock = lock_current_dir();

        // create temp file
        let mut tmp = stdenv::temp_dir();
        tmp.push(format!("cat_test_file_{}", std::process::id()));
        let mut f = fs::File::create(&tmp).expect("create tmp file");
        write!(f, "hello\nworld\n").expect("write");
        drop(f);

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        // Run cat on file
        let cat = Cat {
            files: vec![tmp.to_string_lossy().to_string()],
        };
        let mut out = Vec::new();
        let res = cat.execute(&mut Cursor::new(Vec::new()), &mut out, &mut env);
        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        assert_eq!(s, "hello\nworld\n");

        let _ = fs::remove_file(tmp);
    }

    #[test]
    fn test_cat_reads_stdin_when_no_args() {
        let _lock = lock_current_dir();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        let cat = Cat { files: Vec::new() };
        let input = b"from stdin\nline2\n".to_vec();
        let mut out = Vec::new();
        let res = cat.execute(&mut Cursor::new(input), &mut out, &mut env);
        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        assert_eq!(s, "from stdin\nline2\n");
    }

    #[test]
    fn test_wc_counts_file() {
        let _lock = lock_current_dir();

        // create temp file
        let mut tmp = stdenv::temp_dir();
        tmp.push(format!("wc_test_file_{}", std::process::id()));
        let mut f = fs::File::create(&tmp).expect("create tmp file");
        // two lines, three words, bytes include newlines
        write!(f, "one two\nthree\n").expect("write");
        drop(f);

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        let wc = WC {
            files: vec![tmp.to_string_lossy().to_string()],
        };
        let mut out = Vec::new();
        let res = wc.execute(&mut Cursor::new(Vec::new()), &mut out, &mut env);
        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        // Expect format: "<lines> <words> <bytes> <filename>\n"
        // lines = 2, words = 3, bytes = len("one two\nthree\n") = 14
        let expected_prefix = "2 3 ";
        assert!(s.starts_with(expected_prefix));
        assert!(s.trim_end().ends_with(&tmp.to_string_lossy().to_string()));

        let _ = fs::remove_file(tmp);
    }

    #[test]
    fn test_wc_counts_stdin_when_no_args() {
        let _lock = lock_current_dir();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        let wc = WC { files: Vec::new() };
        let input = b"a b c\n".to_vec(); // 1 line, 3 words, bytes = 6 (including newline)
        let mut out = Vec::new();
        let res = wc.execute(&mut Cursor::new(input), &mut out, &mut env);
        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        assert_eq!(s, "1 3 6\n");
    }

    #[test]
    fn test_wc_multiple_files_output_contains_each_filename() {
        let _lock = lock_current_dir();

        // create two temp files
        let mut tmp1 = stdenv::temp_dir();
        tmp1.push(format!("wc_multi_{}_1", std::process::id()));
        let mut f1 = fs::File::create(&tmp1).unwrap();
        write!(f1, "a b\n").unwrap();
        drop(f1);

        let mut tmp2 = stdenv::temp_dir();
        tmp2.push(format!("wc_multi_{}_2", std::process::id()));
        let mut f2 = fs::File::create(&tmp2).unwrap();
        write!(f2, "c\n").unwrap();
        drop(f2);

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: stdenv::current_dir().unwrap(),
            should_exit: false,
        };

        let wc = WC {
            files: vec![
                tmp1.to_string_lossy().to_string(),
                tmp2.to_string_lossy().to_string(),
            ],
        };
        let mut out = Vec::new();
        let res = wc.execute(&mut Cursor::new(Vec::new()), &mut out, &mut env);
        assert!(res.is_ok());

        let s = String::from_utf8(out).unwrap();
        // Should contain two lines, each ending with filename
        assert!(s.contains(&tmp1.to_string_lossy().to_string()));
        assert!(s.contains(&tmp2.to_string_lossy().to_string()));

        let _ = fs::remove_file(tmp1);
        let _ = fs::remove_file(tmp2);
    }

    use std::io::Error;

    struct TestEnvironment {
        temp_dir: PathBuf,
        file_path: PathBuf,
    }

    /// Создает изолированную временную директорию и файл внутри нее.
    /// Возвращает абсолютный путь к файлу и временную директорию.
    fn setup_test_environment(content: &str) -> Result<TestEnvironment, Error> {
        let mut p = stdenv::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir_name = format!("grep_test_dir_{}_{}", std::process::id(), nanos);
        p.push(dir_name);
        fs::create_dir_all(&p)?;

        let file_path = p.join("test_data.txt");
        let mut f = fs::File::create(&file_path)?;
        write!(f, "{}", content)?;
        drop(f);

        Ok(TestEnvironment {
            temp_dir: p,
            file_path,
        })
    }

    #[test]
    fn test_grep_ignore_case_i_isolated() -> Result<(), anyhow::Error> {
        let env = setup_test_environment("Target 1\nTaRgEt 2\nNo match\n")
            .context("Failed to set up environment")?;

        let mut shell_env = Environment::new();
        let mut out = Vec::new();
        let filename = env.file_path.to_string_lossy().to_string();

        let grep = Grep {
            pattern: "target".to_string(),
            files: vec![filename.clone()],
            word_regexp: false,
            ignore_case: true, // <- -i
            after_context: 0,
        };

        assert_eq!(
            grep.execute(&mut Cursor::new(Vec::new()), &mut out, &mut shell_env)?,
            0
        );
        let s = String::from_utf8(out).unwrap();

        let expected = format!("{}:Target 1\n{}:TaRgEt 2\n", filename, filename);
        assert_eq!(s, expected);

        fs::remove_dir_all(env.temp_dir)?;
        Ok(())
    }

    #[test]
    fn test_grep_trailing_context_a_1_isolated() -> Result<(), anyhow::Error> {
        let content = "Line 1\nMATCH 1\nLine 3\nLine 4\nMATCH 2\nLine 6\nLine 7\nLine 8\n";
        let env = setup_test_environment(content).context("Failed to set up environment")?;

        let mut shell_env = Environment::new();
        let mut out = Vec::new();
        let filename = env.file_path.to_string_lossy().to_string();

        let grep = Grep {
            pattern: "MATCH".to_string(),
            files: vec![filename.clone()],
            word_regexp: false,
            ignore_case: false,
            after_context: 1, // <- -A 1
        };

        assert_eq!(
            grep.execute(&mut Cursor::new(Vec::new()), &mut out, &mut shell_env)?,
            0
        );
        let s = String::from_utf8(out).unwrap();

        let expected = format!(
            "{}:MATCH 1\n\
            {}:Line 3\n\
            --\n\
            {}:MATCH 2\n\
            {}:Line 6\n",
            filename, filename, filename, filename
        );

        assert_eq!(s, expected);

        fs::remove_dir_all(env.temp_dir)?;
        Ok(())
    }

    #[test]
    fn test_grep_context_overlap_a_2_isolated() -> Result<(), anyhow::Error> {
        let content = "MATCH 1\nLine 2\nMATCH 2\nLine 4\nLine 5\nLine 6\n";
        let env = setup_test_environment(content).context("Failed to set up environment")?;

        let mut shell_env = Environment::new();
        let mut out = Vec::new();
        let filename = env.file_path.to_string_lossy().to_string();

        let grep = Grep {
            pattern: "MATCH".to_string(),
            files: vec![filename.clone()],
            word_regexp: false,
            ignore_case: false,
            after_context: 2, // <- -A 2
        };

        assert_eq!(
            grep.execute(&mut Cursor::new(Vec::new()), &mut out, &mut shell_env)?,
            0
        );
        let s = String::from_utf8(out).unwrap();

        let expected = format!(
            "{}:MATCH 1\n\
            {}:Line 2\n\
            {}:MATCH 2\n\
            {}:Line 4\n\
            {}:Line 5\n",
            filename, filename, filename, filename, filename
        );

        assert_eq!(s, expected);

        fs::remove_dir_all(env.temp_dir)?;
        Ok(())
    }

    #[test]
    fn test_grep_stdin_pipe_isolated() -> Result<(), anyhow::Error> {
        let mut shell_env = Environment::new();
        let mut out = Vec::new();

        let grep = Grep {
            pattern: "pipe".to_string(),
            files: Vec::new(), // <- stdin
            word_regexp: false,
            ignore_case: false,
            after_context: 0,
        };

        let input = b"Line 1\nLine with pipe target\nLine 3\n".to_vec();

        assert_eq!(
            grep.execute(&mut Cursor::new(input), &mut out, &mut shell_env)?,
            0
        );
        let s = String::from_utf8(out).unwrap();

        // При чтении из stdin префикс файла отсутствует
        assert_eq!(s, "Line with pipe target\n");
        Ok(())
    }
}
