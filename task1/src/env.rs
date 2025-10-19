use std::collections::HashMap;
use std::env as stdenv;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Environment {
    pub vars: HashMap<String, String>,
    pub current_dir: PathBuf,
    pub should_exit: bool,
}

impl Environment {
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

    pub fn get_var(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .cloned()
            .or_else(|| stdenv::var(key).ok())
    }

    pub fn set_var(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.vars.insert(key.into(), val.into());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::{
        env as stdenv, fs, io,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::builtin::{Cd, Echo, Pwd};
    use crate::command::ShellCommand;
    use crate::env::Environment;

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

    #[test]
    fn test_pwd_prints_current_dir() {
        let cur = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: cur.clone(),
            should_exit: false,
        };

        let mut out = Vec::new();
        let cmd = Pwd::new();
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
        let echo1 = Echo::new(vec!["hello".to_string(), "world".to_string()], true);
        let res1 = echo1.execute(&mut Cursor::new(Vec::new()), &mut out1, &mut env);

        assert!(res1.is_ok());
        assert_eq!(String::from_utf8(out1).unwrap(), "hello world\n");

        // Without newline
        let mut out2 = Vec::new();
        let echo2 = Echo::new(vec!["foo".to_string(), "bar".to_string()], false);
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
        let temp = make_unique_temp_dir().expect("failed to create temp dir");
        let canonical_temp = fs::canonicalize(&temp).expect("canonicalize failed");

        // save original cwd to restore later
        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        let cmd = Cd::new(Some(canonical_temp.to_string_lossy().to_string()));
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
        let temp = make_unique_temp_dir().expect("failed to create temp dir");
        let canonical_temp = fs::canonicalize(&temp).expect("canonicalize failed");

        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        env.set_var("HOME", canonical_temp.to_string_lossy().to_string());

        let cmd = Cd::new(None);
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
        let orig = stdenv::current_dir().unwrap();

        let mut env = Environment {
            vars: HashMap::new(),
            current_dir: orig.clone(),
            should_exit: false,
        };

        let name = format!("nonexistent_dir_for_task1_test_{}", std::process::id());
        let cmd = Cd::new(Some(name));
        let res = cmd.execute(&mut Cursor::new(Vec::new()), &mut Vec::new(), &mut env);

        assert!(res.is_err());
        assert_eq!(stdenv::current_dir().unwrap(), orig);
    }
}
