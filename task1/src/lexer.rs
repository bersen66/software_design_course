//! A module implementing lexical analysis (tokenization) for a simple shell-like language.

/// A part of a word, which can be either literal text, a command substitution, or a parameter substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordPart {
    /// Literal text that requires no further processing.
    Literal(String),
    /// Command substitution in the format `$(...)`. Contains the text inside the parentheses.
    CmdSubst(String),
    /// Parameter substitution in the format `${...}`. Contains the text inside the curly braces.
    ParamSubst(String),
}

/// Represents a token resulting from lexical analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A word token, which may be composed of multiple parts (`WordPart`).
    Word(Vec<WordPart>),
    /// The pipe operator, `|`.
    PipeOp,
    /// The equality symbol, `=`.
    Equal,
    /// The dot symbol, `.`.
    Dot,
    /// The slash symbol (path separator), `/`.
    Slash,
    /// Input redirection symbol, `<`.
    RedirectLeft,
    /// Output redirection symbol, `>`.
    RedirectRight,
}

/// Errors that can occur during the lexical analysis process.
#[derive(Debug)]
pub enum LexingError {
    /// A closing quote (single or double) was not found.
    UnfinishedQuote,
    /// A closing parenthesis for command substitution `$(...)` was not found.
    UnfinishedCmdSubst,
    /// A closing brace for parameter substitution `${...}` was not found.
    UnfinishedParamSubst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexingState {
    Start,
    ReadingWord,
    ReadingSingleQuote,
    ReadingDoubleQuote,
    ReadingCmdSubst(usize),   // nesting depth
    ReadingParamSubst(usize), // nesting depth
}

struct LexingFSM {
    input: Vec<char>,
    pos: usize,
    state: LexingState,
    current_word: Vec<WordPart>,
    buffer: String,
}

impl LexingFSM {
    /// Creates a new instance of the lexical analysis Finite State Machine.
    ///
    /// # Arguments
    /// * `line` - The input string to be lexed.
    fn new(line: String) -> Self {
        LexingFSM {
            input: line.chars().collect(),
            pos: 0,
            state: LexingState::Start,
            current_word: Vec::new(),
            buffer: String::new(),
        }
    }

    /// Performs lexical analysis on the input string and returns a vector of tokens.
    ///
    /// The method iterates through the input, updating the FSM's state and accumulating
    /// tokens based on the shell's tokenization rules, including handling quotes and substitutions.
    ///
    /// # Returns
    /// A `Result<Vec<Token>, LexingError>`: A vector of tokens on success, or a `LexingError`
    /// if an incomplete structure (like an unclosed quote) is found.
    fn make_tokens(&mut self) -> Result<Vec<Token>, LexingError> {
        let mut out = Vec::new();

        while let Some(ch) = self.read_char() {
            match self.state {
                LexingState::Start => self.handle_start(ch, &mut out)?,
                LexingState::ReadingWord => self.handle_word(ch, &mut out)?,
                LexingState::ReadingSingleQuote => self.handle_single_quote(ch)?,
                LexingState::ReadingDoubleQuote => self.handle_double_quote(ch)?,
                LexingState::ReadingCmdSubst(depth) => self.handle_cmdsubst(ch, depth)?,
                LexingState::ReadingParamSubst(depth) => self.handle_paramsubst(ch, depth)?,
            }
        }

        // If something remains
        match self.state {
            LexingState::ReadingSingleQuote => return Err(LexingError::UnfinishedQuote),
            LexingState::ReadingDoubleQuote => return Err(LexingError::UnfinishedQuote),
            LexingState::ReadingCmdSubst(_) => return Err(LexingError::UnfinishedCmdSubst),
            LexingState::ReadingParamSubst(_) => return Err(LexingError::UnfinishedParamSubst),
            _ => {}
        }

        self.finalize_current_word_part()?;
        if !self.current_word.is_empty() {
            out.push(Token::Word(std::mem::take(&mut self.current_word)));
        }

        Ok(out)
    }

    fn read_char(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn peek_char(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn handle_start(&mut self, ch: char, out: &mut Vec<Token>) -> Result<(), LexingError> {
        match ch {
            ' ' | '\t' => {}
            '|' => out.push(Token::PipeOp),
            '=' => out.push(Token::Equal),
            '.' => out.push(Token::Dot),
            '/' => out.push(Token::Slash),
            '<' => out.push(Token::RedirectLeft),
            '>' => out.push(Token::RedirectRight),
            '\'' => self.state = LexingState::ReadingSingleQuote,
            '"' => self.state = LexingState::ReadingDoubleQuote,
            '$' => {
                if self.peek_char() == Some('(') {
                    self.read_char();
                    self.state = LexingState::ReadingCmdSubst(1);
                } else if self.peek_char() == Some('{') {
                    self.read_char();
                    self.state = LexingState::ReadingParamSubst(1);
                } else {
                    // Start of a simple parameter substitution like $a
                    if !self.buffer.is_empty() {
                        self.current_word
                            .push(WordPart::Literal(std::mem::take(&mut self.buffer)));
                    }
                    self.state = LexingState::ReadingWord;
                    // Don't push the $ yet, we'll handle it in handle_word
                    self.buffer.push('$');
                }
            }
            c => {
                self.buffer.push(c);
                self.state = LexingState::ReadingWord;
            }
        }
        Ok(())
    }

    fn handle_word(&mut self, ch: char, out: &mut Vec<Token>) -> Result<(), LexingError> {
        match ch {
            ' ' | '\t' => {
                self.finalize_current_word_part()?;
                out.push(Token::Word(std::mem::take(&mut self.current_word)));
                self.state = LexingState::Start;
            }
            '|' | '=' | '/' | '.' | '<' | '>' => {
                // Finalize the current word
                self.finalize_current_word_part()?;
                if !self.current_word.is_empty() {
                    out.push(Token::Word(std::mem::take(&mut self.current_word)));
                }
                // Add the symbol token
                let token = match ch {
                    '|' => Token::PipeOp,
                    '=' => Token::Equal,
                    '.' => Token::Dot,
                    '/' => Token::Slash,
                    '<' => Token::RedirectLeft,
                    '>' => Token::RedirectRight,
                    _ => unreachable!(),
                };
                out.push(token);
                self.state = LexingState::Start;
            }
            '"' => self.state = LexingState::ReadingDoubleQuote,
            '\'' => self.state = LexingState::ReadingSingleQuote,
            '$' => {
                if self.peek_char() == Some('(') {
                    self.read_char();
                    self.finalize_current_word_part()?;
                    let nested = self.collect_cmdsubst(1)?;
                    self.current_word.push(WordPart::CmdSubst(nested));
                } else if self.peek_char() == Some('{') {
                    self.read_char();
                    self.finalize_current_word_part()?;
                    let nested = self.collect_paramsubst(1)?;
                    self.current_word.push(WordPart::ParamSubst(nested));
                } else {
                    // This is a simple parameter substitution like $a
                    self.finalize_current_word_part()?;
                    self.buffer.push('$');
                }
            }
            c => {
                // Check if we're starting a simple parameter substitution
                if !self.buffer.is_empty() && self.buffer == "$" {
                    // We have a $ followed by a valid parameter name character
                    if c.is_alphabetic() || c == '_' {
                        // Continue collecting the parameter name
                        self.buffer.push(c);
                    } else {
                        // Not a valid parameter name, treat as literal
                        self.buffer.push(c);
                    }
                } else {
                    self.buffer.push(c);
                }
            }
        }
        Ok(())
    }

    fn handle_single_quote(&mut self, ch: char) -> Result<(), LexingError> {
        match ch {
            '\'' => {
                self.current_word
                    .push(WordPart::Literal(std::mem::take(&mut self.buffer)));
                self.state = LexingState::ReadingWord
            }
            c => self.buffer.push(c),
        }
        Ok(())
    }

    fn handle_double_quote(&mut self, ch: char) -> Result<(), LexingError> {
        match ch {
            '"' => {
                self.finalize_current_word_part()?;
                self.state = LexingState::ReadingWord;
            }
            '$' if self.peek_char() == Some('(') => {
                self.read_char();
                self.finalize_current_word_part()?;
                let nested = self.collect_cmdsubst(1)?;
                self.current_word.push(WordPart::CmdSubst(nested));
            }
            '$' if self.peek_char() == Some('{') => {
                self.read_char();
                self.finalize_current_word_part()?;
                let nested = self.collect_paramsubst(1)?;
                self.current_word.push(WordPart::ParamSubst(nested));
            }
            '$' => {
                // Simple parameter substitution in double quotes
                self.finalize_current_word_part()?;
                self.buffer.push('$');
            }
            c => self.buffer.push(c),
        }
        Ok(())
    }

    fn handle_cmdsubst(&mut self, ch: char, depth: usize) -> Result<(), LexingError> {
        match ch {
            '(' => {
                self.buffer.push(ch);
                self.state = LexingState::ReadingCmdSubst(depth + 1);
            }
            ')' if depth == 1 => {
                self.current_word
                    .push(WordPart::CmdSubst(std::mem::take(&mut self.buffer)));
                self.state = LexingState::ReadingWord;
            }
            ')' => {
                self.buffer.push(ch);
                self.state = LexingState::ReadingCmdSubst(depth - 1);
            }
            c => self.buffer.push(c),
        }
        Ok(())
    }

    fn handle_paramsubst(&mut self, ch: char, depth: usize) -> Result<(), LexingError> {
        match ch {
            '{' => {
                self.buffer.push(ch);
                self.state = LexingState::ReadingParamSubst(depth + 1);
            }
            '}' if depth == 1 => {
                self.current_word
                    .push(WordPart::ParamSubst(std::mem::take(&mut self.buffer)));
                self.state = LexingState::ReadingWord;
            }
            '}' => {
                self.buffer.push(ch);
                self.state = LexingState::ReadingParamSubst(depth - 1);
            }
            c => self.buffer.push(c),
        }
        Ok(())
    }

    /// Recursively collects characters within a command substitution block `$(...)`.
    /// Handles nested parentheses by tracking the `depth`.
    fn collect_cmdsubst(&mut self, mut depth: usize) -> Result<String, LexingError> {
        let mut s = String::new();
        while let Some(ch) = self.read_char() {
            match ch {
                '(' => {
                    depth += 1;
                    s.push(ch);
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(s);
                    }
                    s.push(ch);
                }
                _ => s.push(ch),
            }
        }
        Err(LexingError::UnfinishedCmdSubst)
    }

    /// Recursively collects characters within a parameter substitution block `${...}`.
    /// Handles nested braces by tracking the `depth`.
    fn collect_paramsubst(&mut self, mut depth: usize) -> Result<String, LexingError> {
        let mut s = String::new();
        while let Some(ch) = self.read_char() {
            match ch {
                '{' => {
                    depth += 1;
                    s.push(ch);
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(s);
                    }
                    s.push(ch);
                }
                _ => s.push(ch),
            }
        }
        Err(LexingError::UnfinishedParamSubst)
    }

    /// Helper method to finalize the current buffer as either Literal or ParamSubst
    fn finalize_current_word_part(&mut self) -> Result<(), LexingError> {
        if !self.buffer.is_empty() {
            if self.buffer.starts_with('$') && self.buffer.len() > 1 {
                // This is a simple parameter substitution like $a or $var
                let param_name = self.buffer[1..].to_string();
                if !param_name.is_empty() && (param_name.chars().next().unwrap().is_alphabetic() || param_name.chars().next().unwrap() == '_') {
                    self.current_word.push(WordPart::ParamSubst(param_name));
                } else {
                    // Not a valid parameter name, treat as literal
                    self.current_word.push(WordPart::Literal(self.buffer.clone()));
                }
            } else {
                // Regular literal
                self.current_word.push(WordPart::Literal(std::mem::take(&mut self.buffer)));
            }
            self.buffer.clear();
        }
        Ok(())
    }
}

/// The main entry point function to perform lexical analysis.
///
/// Creates and runs the finite state machine to tokenize the input line.
///
/// # Arguments
/// * `line` - The string to be tokenized.
///
/// # Returns
/// `Result<Vec<Token>, LexingError>`: A vector of tokens on success, or a `LexingError`
/// if an incomplete structure is encountered.
pub fn split_into_tokens(line: String) -> Result<Vec<Token>, LexingError> {
    let mut lexer = LexingFSM::new(line);
    lexer.make_tokens()
}
