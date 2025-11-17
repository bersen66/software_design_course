use std::cell::RefCell;
use std::io::{Cursor, Read, Result as IoResult, Write};
use std::rc::Rc;
use std::process::Stdio;

/// Memory-backed reader for builtins.
///
/// Public so it can be constructed from other modules.
pub struct MemReader {
    cursor: Cursor<Vec<u8>>,
}

impl MemReader {
    /// Create a MemReader that will read from the provided buffer.
    pub fn new(buf: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(buf),
        }
    }
}

impl Read for MemReader {
    fn read(&mut self, out: &mut [u8]) -> IoResult<usize> {
        self.cursor.read(out)
    }
}

impl crate::command::Stdin for MemReader {
    /// For in-memory reader return Stdio::null() because this adapter is used
    /// only for builtin commands executed in-process.
    fn stdio(self: Box<Self>) -> Stdio {
        Stdio::null()
    }
}

/// Memory-backed writer for capturing stdout from builtins.
pub struct MemWriter {
    buf: Rc<RefCell<Vec<u8>>>,
}

impl MemWriter {
    /// Public constructor.
    pub fn new() -> Self {
        Self {
            buf: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Return inner Rc so caller can read collected bytes after command execution.
    pub fn into_inner(self) -> Rc<RefCell<Vec<u8>>> {
        self.buf
    }

    /// Convenience: create writer and return (writer, rc_handle).
    pub fn with_handle() -> (Self, Rc<RefCell<Vec<u8>>>) {
        let mw = MemWriter::new();
        let rc = mw.buf.clone();
        (mw, rc)
    }
}

impl Write for MemWriter {
    fn write(&mut self, data: &[u8]) -> IoResult<usize> {
        self.buf.borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl crate::command::Stdout for MemWriter {
    fn stdio(self: Box<Self>) -> Stdio {
        Stdio::null()
    }
}
