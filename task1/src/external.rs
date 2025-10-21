use crate::command::{CommandFactory, ExecutableCommand, ExitCode, Stdin, Stdout};
use crate::env::Environment;
use anyhow::Result;
use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use crate::interpreter::Factory;

/// Command that is not a builtin.
pub struct ExternalCommand {
    name: OsString,
    args: Vec<OsString>,
}

impl ExternalCommand {
    pub fn new(name: OsString, args: Vec<OsString>) -> Self {
        Self { name, args }
    }
}

impl CommandFactory for Factory<ExternalCommand> {
    fn try_create(
        &self,
        env: &Environment,
        name: &str,
        args: &[&str],
    ) -> Option<Box<dyn ExecutableCommand>> {
        let search_paths = env.get_var("PATH")?;
        match find_command_path(OsStr::new(&search_paths), Path::new(&name)) {
            Some(executable) => Some(Box::new(ExternalCommand::new(
                executable.as_os_str().to_owned(),
                args.iter().map(|x| x.into()).collect(),
            ))),
            None => None,
        }
    }
}

impl ExecutableCommand for ExternalCommand {
    fn execute(
        self: Box<Self>,
        stdin: Box<dyn Stdin>,
        stdout: Box<dyn Stdout>,
        env: &mut Environment,
    ) -> Result<ExitCode> {
        let mut cmd = std::process::Command::new(&self.name)
            .args(&self.args)
            .stdin(stdin.stdio())
            .stdout(stdout.stdio())
            .envs(env.vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .current_dir(&env.current_dir)
            .spawn()?;
        let exit_status = cmd.wait()?;
        match exit_status.code() {
            Some(x) => Ok(x),
            None => Ok(terminated_by_signal(exit_status)),
        }
    }
}

#[cfg(unix)]
fn terminated_by_signal(exit_status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    if let Some(signal) = ExitStatusExt::signal(&exit_status) {
        128 + signal
    } else if ExitStatusExt::core_dumped(&exit_status) {
        255
    } else {
        -1
    }
}

#[cfg(not(unix))]
fn terminated_by_signal(_exit_status: ExitStatus) -> i32 {
    -1
}

/// Resolve a command path the way a typical shell would.
///
/// Behavior:
/// - Absolute path: returns it if it exists.
/// - Relative with multiple components (e.g., `bin/sh`): returns it if it exists.
/// - `./foo` on Unix or any `./`-prefixed path on other platforms: returns it if it exists.
/// - Single path component (no separators): search each directory in `search_paths` (PATH)
///   and return the first existing match.
/// - Empty path: returns `None`.
///
/// Returns either a borrowed reference to the provided `path` or an owned `PathBuf`
/// when the result is discovered via PATH lookup.
pub fn find_command_path<'a>(search_paths: &OsStr, path: &'a Path) -> Option<Cow<'a, Path>> {
    if path.is_absolute() {
        return find_by_path(path).map(Cow::Borrowed);
    }

    let search_in_current_dir = cfg!(not(unix)) || path.starts_with("./");
    if search_in_current_dir && path.exists() {
        return Some(Cow::Borrowed(path));
    }

    let mut components = path.components();
    let first = components.next();
    let second = components.next();
    match (first, second) {
        (None, None) => {
            // Empty path -> not found
            None
        }
        (Some(x), None) => {
            // Single component -> search in PATH
            find_in_path(search_paths, x.as_os_str()).map(Cow::Owned)
        }
        _ => {
            // Multiple components -> search in current dir
            find_by_path(path).map(Cow::Borrowed)
        }
    }
}

fn find_in_path(search_paths: &OsStr, cmd: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(search_paths) {
        let path = dir.join(cmd);
        if let Some(path) = find_by_path(&path) {
            return Some(path.to_owned());
        }
    }
    None
}

fn find_by_path(path: &Path) -> Option<&Path> {
    if path.exists() { Some(path) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::fs::File;

    #[cfg(unix)]
    fn osstr(s: &str) -> &OsStr {
        OsStr::new(s)
    }

    #[test]
    #[cfg(unix)]
    fn absolute_existing_true() {
        let path = Path::new("/bin/sh");
        let res = find_command_path(osstr("/bin"), path);
        assert!(res.is_some(), "Expected to find /bin/sh via absolute path");
        let found = res.unwrap();
        assert_eq!(found.as_ref(), path);
    }

    #[test]
    #[cfg(unix)]
    fn absolute_nonexisting() {
        let path = Path::new("/bin/nonexisting");
        let res = find_command_path(osstr("/bin"), path);
        assert!(
            res.is_none(),
            "Expected not to find /bin/nonexisting via absolute path"
        );
    }

    #[test]
    #[cfg(unix)]
    fn single_component_found_in_path() {
        // Search for "sh" in PATH that includes /bin
        let path = Path::new("sh");
        let res = find_command_path(osstr("/bin"), path);
        let found = res.expect("Expected to find 'sh' in /bin via PATH search");
        assert!(
            found.as_ref().ends_with("sh"),
            "Found path should end with 'sh' but was {:?}",
            found
        );
        assert!(
            found.as_ref().starts_with("/bin"),
            "Expected path in /bin, got {:?}",
            found
        );
    }

    #[test]
    #[cfg(unix)]
    fn single_component_not_found_in_path() {
        let path = Path::new("nonexisting");
        let res = find_command_path(osstr("/bin"), path);
        assert!(res.is_none(), "Expected not to find 'nonexisting' in PATH");
    }

    #[test]
    #[cfg(unix)]
    fn multiple_components_relative_existing() {
        // Create a temporary working directory with a nested file: bin/sh
        let cwd_before = std::env::current_dir().expect("cwd");
        let tmp_base =
            std::env::temp_dir().join(format!("external_tests_{}_mc", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_base);
        fs::create_dir_all(tmp_base.join("bin")).expect("create temp bin dir");
        let file_path = tmp_base.join("bin").join("sh");
        File::create(&file_path).expect("touch bin/sh");

        std::env::set_current_dir(&tmp_base).expect("set cwd");
        let res = find_command_path(osstr("/does/not/matter"), Path::new("bin/sh"));
        // Restore cwd early to avoid interference even on failure
        std::env::set_current_dir(&cwd_before).ok();

        let found = res.expect("Expected to find relative 'bin/sh' in current dir");
        assert!(found.as_ref().ends_with("bin/sh"));
        // Clean up
        let _ = fs::remove_dir_all(tmp_base);
    }

    #[test]
    #[cfg(unix)]
    fn current_dir_with_dot_prefix() {
        // Create a temporary working directory with a file: ./foo
        let cwd_before = std::env::current_dir().expect("cwd");
        let tmp_base =
            std::env::temp_dir().join(format!("external_tests_{}_dot", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_base);
        fs::create_dir_all(&tmp_base).expect("create temp dir");
        let file_path = tmp_base.join("foo");
        File::create(&file_path).expect("touch foo");

        std::env::set_current_dir(&tmp_base).expect("set cwd");
        let res = find_command_path(osstr("/bin"), Path::new("./foo"));
        // Restore cwd
        std::env::set_current_dir(&cwd_before).ok();

        let found = res.expect("Expected to find './foo' in current dir");
        assert_eq!(found.as_ref(), Path::new("./foo"));
        // Clean up
        let _ = fs::remove_dir_all(tmp_base);
    }

    #[test]
    #[cfg(unix)]
    fn empty_path_is_none() {
        let res = find_command_path(osstr("/bin"), Path::new(""));
        assert!(res.is_none(), "Empty path should not resolve to anything");
    }
}
