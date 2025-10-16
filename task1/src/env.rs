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
