#![allow(dead_code, unused_variables, unused_imports)]
use crate::lexer::{Token, WordPart};

/// A shell word, either a simple literal or a compound (with substitutions)
#[derive(Debug, Clone)]
pub enum Word {
    Literal(String),
    Compound(Vec<WordPart>),
}

/// AST node for the shell
#[derive(Debug)]
pub enum AstNode {
    /// A pipeline of commands connected by `|`
    Pipeline(Vec<AstNode>),

    /// A simple command: argv, assignments, redirects
    Command {
        argv: Vec<Word>,
        assignments: Vec<AstNode>, // Assignment nodes
        redirects: Vec<AstNode>,   // Redirect nodes
    },

    /// Variable assignment: name=value
    Assignment { name: String, value: Option<Word> },

    /// I/O redirection
    Redirect { kind: RedirectKind, target: Word },

    /// Substitution: either $(...) or ${...}
    Substitution {
        kind: SubstKind,
        content: Box<AstNode>, // fully parsed AST of the inner command/pipeline
    },
}

/// Kind of redirection
#[derive(Debug)]
pub enum RedirectKind {
    Input,
    Output,
    Append,
}

/// Kind of substitution
#[derive(Debug)]
pub enum SubstKind {
    Command,   // $(...)
    Parameter, // ${...}
}

#[derive(Debug)]
pub enum ParsingError {
    UnexpectedToken(Token),
    UnexpectedEnd,
    ExpectedWord,
    ExpectedAssignmentName,
    InvalidAssignment,
    EmptyPipeline,
    UnsupportedSubstitution, // For substitutions we haven't implemented yet
}

struct AstBuilder {
    tokens: Vec<Token>,
    pos: usize,
}

impl AstBuilder {
    fn from(tokens: Vec<Token>) -> Self {
        AstBuilder { tokens, pos: 0 }
    }

    fn build_ast(mut self) -> Result<AstNode, ParsingError> {
        let ast = self.parse_pipeline()?;

        // Ensure we consumed all tokens
        if self.pos < self.tokens.len() {
            return Err(ParsingError::UnexpectedToken(self.tokens[self.pos].clone()));
        }

        Ok(ast)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn consume(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParsingError> {
        match self.consume() {
            Some(token) if token == expected => Ok(()),
            Some(token) => Err(ParsingError::UnexpectedToken(token)),
            None => Err(ParsingError::UnexpectedEnd),
        }
    }

    /// Parse a pipeline: command ('|' command)*
    fn parse_pipeline(&mut self) -> Result<AstNode, ParsingError> {
        let mut commands = Vec::new();

        // Parse first command
        commands.push(self.parse_command()?);

        // Parse additional commands separated by pipes
        while let Some(Token::PipeOp) = self.peek() {
            self.consume(); // consume the '|'
            commands.push(self.parse_command()?);
        }

        if commands.len() == 1 {
            Ok(commands.remove(0))
        } else {
            Ok(AstNode::Pipeline(commands))
        }
    }

    /// Parse a command: (assignment* word* redirect*)
    // In parser.rs, replace the parse_command method with this improved version:

    /// Parse a command: (assignment* word* redirect*)
    fn parse_command(&mut self) -> Result<AstNode, ParsingError> {
        let mut assignments = Vec::new();
        let mut argv = Vec::new();
        let mut redirects = Vec::new();

        // Parse tokens until we hit a pipe or end
        while let Some(token) = self.peek() {
            match token {
                Token::PipeOp => break, // End of command
                Token::Equal => return Err(ParsingError::UnexpectedToken(token.clone())),
                Token::Word(_parts) => {
                    // Check if this might be an assignment (word followed by =)
                    if let Some(Token::Equal) = self.peek_n(1) {
                        assignments.push(self.parse_assignment()?);
                    } else {
                        argv.push(self.parse_word()?);
                    }
                }
                Token::RedirectLeft => {
                    redirects.push(self.parse_redirect()?);
                }
                Token::RedirectRight => {
                    // Check for append redirect '>>'
                    if let Some(Token::RedirectRight) = self.peek_n(1) {
                        self.consume(); // consume first '>'
                        self.consume(); // consume second '>'
                        let target = self.parse_word()?;
                        redirects.push(AstNode::Redirect {
                            kind: RedirectKind::Append,
                            target,
                        });
                    } else {
                        redirects.push(self.parse_redirect()?);
                    }
                }
                Token::Dot | Token::Slash => {
                    // Parse dots and slashes as part of paths
                    argv.push(self.parse_path()?);
                }
            }
        }

        // A command must have at least something (argv, assignment, or redirect)
        if assignments.is_empty() && argv.is_empty() && redirects.is_empty() {
            return Err(ParsingError::EmptyPipeline);
        }

        Ok(AstNode::Command {
            argv,
            assignments,
            redirects,
        })
    }

    /// Parse a path by combining consecutive dots, slashes, and words
    fn parse_path(&mut self) -> Result<Word, ParsingError> {
        let mut path_parts = Vec::new();

        // Collect all consecutive path-related tokens
        while let Some(token) = self.peek() {
            match token {
                Token::Word(parts) => {
                    path_parts.extend(parts.clone());
                    self.consume();
                }
                Token::Dot => {
                    path_parts.push(WordPart::Literal(".".to_string()));
                    self.consume();
                }
                Token::Slash => {
                    path_parts.push(WordPart::Literal("/".to_string()));
                    self.consume();
                }
                _ => break,
            }
        }

        if path_parts.is_empty() {
            return Err(ParsingError::ExpectedWord);
        }

        // If we have only one literal part, return it as a simple Literal
        if path_parts.len() == 1 {
            if let WordPart::Literal(s) = &path_parts[0] {
                return Ok(Word::Literal(s.clone()));
            }
        }

        Ok(Word::Compound(path_parts))
    }
    
    /// Parse an assignment: word '=' word?
    fn parse_assignment(&mut self) -> Result<AstNode, ParsingError> {
        // Get the name (must be a simple literal word)
        let name_word = match self.consume() {
            Some(Token::Word(parts)) => Self::word_parts_to_ast_word(parts)?,
            Some(token) => return Err(ParsingError::UnexpectedToken(token)),
            None => return Err(ParsingError::UnexpectedEnd),
        };

        // Extract the name as string (must be a literal)
        let name = match name_word {
            Word::Literal(s) => s,
            Word::Compound(_) => return Err(ParsingError::InvalidAssignment),
        };

        // Validate name (simple check for valid variable name)
        if name.is_empty() || !name.chars().next().unwrap().is_alphabetic() {
            return Err(ParsingError::ExpectedAssignmentName);
        }

        // Consume the '='
        self.expect(Token::Equal)?;

        // Parse the value if present
        let value = match self.peek() {
            Some(Token::Word(_)) => Some(self.parse_word()?),
            Some(Token::PipeOp) | None => None,
            Some(token) => return Err(ParsingError::UnexpectedToken(token.clone())),
        };

        Ok(AstNode::Assignment { name, value })
    }

    /// Parse a redirect: '<' word or '>' word
    fn parse_redirect(&mut self) -> Result<AstNode, ParsingError> {
        let kind = match self.consume() {
            Some(Token::RedirectLeft) => RedirectKind::Input,
            Some(Token::RedirectRight) => RedirectKind::Output,
            Some(token) => return Err(ParsingError::UnexpectedToken(token)),
            None => return Err(ParsingError::UnexpectedEnd),
        };

        // Parse the target word
        let target = self.parse_word()?;

        Ok(AstNode::Redirect { kind, target })
    }

    /// Parse a word from the current token
    fn parse_word(&mut self) -> Result<Word, ParsingError> {
        match self.consume() {
            Some(Token::Word(parts)) => Self::word_parts_to_ast_word(parts),
            Some(Token::Dot) => Ok(Word::Literal(".".to_string())),
            Some(Token::Slash) => Ok(Word::Literal("/".to_string())),
            Some(token) => Err(ParsingError::UnexpectedToken(token)),
            None => Err(ParsingError::UnexpectedEnd),
        }
    }

    /// Handle command and parameter substitutions within words
    fn handle_substitutions_in_word(&mut self, parts: Vec<WordPart>) -> Result<Word, ParsingError> {
        let mut processed_parts = Vec::new();

        for part in parts {
            match part {
                WordPart::CmdSubst(content) => {
                    // For now, we'll just return an error for command substitutions
                    // In a full implementation, you'd recursively parse the content
                    return Err(ParsingError::UnsupportedSubstitution);
                }
                WordPart::ParamSubst(content) => {
                    // For now, we'll just return an error for parameter substitutions
                    // In a full implementation, you'd parse the parameter substitution syntax
                    return Err(ParsingError::UnsupportedSubstitution);
                }
                WordPart::Literal(text) => {
                    processed_parts.push(WordPart::Literal(text));
                }
            }
        }

        if processed_parts.len() == 1 {
            if let WordPart::Literal(text) = &processed_parts[0] {
                Ok(Word::Literal(text.clone()))
            } else {
                Ok(Word::Compound(processed_parts))
            }
        } else {
            Ok(Word::Compound(processed_parts))
        }
    }

    /// Helper to look ahead n tokens
    fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// Convert lexer WordParts to AST Word
    fn word_parts_to_ast_word(parts: Vec<WordPart>) -> Result<Word, ParsingError> {
        if parts.len() == 1 {
            if let WordPart::Literal(s) = &parts[0] {
                return Ok(Word::Literal(s.clone()));
            }
        }
        Ok(Word::Compound(parts))
    }
}

pub fn construct_ast(tokens: Vec<Token>) -> Result<AstNode, ParsingError> {
    let builder = AstBuilder::from(tokens);
    builder.build_ast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexing::split_into_tokens;

    #[test]
    fn test_simple_command() {
        let tokens = split_into_tokens("ls -l".to_string()).unwrap();
        let ast = construct_ast(tokens).unwrap();

        if let AstNode::Command {
            argv,
            assignments,
            redirects,
        } = ast
        {
            assert_eq!(argv.len(), 2);
            assert!(assignments.is_empty());
            assert!(redirects.is_empty());
        } else {
            panic!("Expected Command node");
        }
    }

    #[test]
    fn test_assignment() {
        let tokens = split_into_tokens("a=hello".to_string()).unwrap();
        let ast = construct_ast(tokens).unwrap();

        if let AstNode::Command { assignments, .. } = ast {
            assert_eq!(assignments.len(), 1);
            if let AstNode::Assignment { name, value } = &assignments[0] {
                assert_eq!(name, "a");
                assert!(value.is_some());
            }
        } else {
            panic!("Expected Command node with assignment");
        }
    }
}
