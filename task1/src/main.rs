use shell_commands::Interpreter;

fn main() {
    println!("$ echo --help");
    Interpreter::default().run("echo", &["--help"]).unwrap();

    println!("\n$ echo hello world");
    Interpreter::default()
        .run("echo", &["hello", "world!"])
        .unwrap();

    println!("\n$ echo -n foo; echo -n bar; echo baz");
    Interpreter::default().run("echo", &["-n", "foo"]).unwrap();
    Interpreter::default().run("echo", &["-n", "bar"]).unwrap();
    Interpreter::default().run("echo", &["baz"]).unwrap();

    println!("\n$ pwd");
    Interpreter::default().run("pwd", &[]).unwrap();

    println!("\n$ cd src");
    Interpreter::default().run("cd", &["./src"]).unwrap();

    println!("\n$ pwd");
    Interpreter::default().run("pwd", &[]).unwrap();

    println!("\n$ /usr/bin/true");
    Interpreter::default().run("/usr/bin/true", &[]).unwrap();

    println!("\n$ /usr/bin/false");
    Interpreter::default().run("/usr/bin/false", &[]).unwrap();
}
