use shell_commands::Interpreter;

fn main() {
    let mut interpreter = Interpreter::default();
    let _ = interpreter.repl().unwrap();
}
