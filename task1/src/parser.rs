#![allow(dead_code, unused_variables, unused_imports)]
use crate::lexer::{Token, WordPart};

/// A shell word, either a simple literal or a compound (with substitutions)
#[derive(Debug, Clone, PartialEq)] 
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
    fn parse_command(&mut self) -> Result<AstNode, ParsingError> {
        let mut assignments = Vec::new();
        let mut argv = Vec::new();
        let mut redirects = Vec::new();

        // Parse tokens until we hit a pipe or end
        while let Some(token) = self.peek() {
            match token {
                Token::PipeOp => break, // End of command
                Token::Equal => return Err(ParsingError::UnexpectedToken(token.clone())),

                Token::Word(parts) => {
                    let is_potential_assignment = matches!(self.peek_n(1), Some(Token::Equal));
                    // Check if the next token is a Slash, indicating a path needs parsing
                    let is_path_start = matches!(self.peek_n(1), Some(Token::Slash));

                    if argv.is_empty() && is_potential_assignment {
                        
                        // Check if it's a valid shell variable name start (starts with a letter)
                        let is_valid_name_start = parts.len() == 1 
                            && matches!(&parts[0], WordPart::Literal(s) if s.chars().next().map_or(false, |c| c.is_ascii_alphabetic()));

                        if is_valid_name_start {
                            // Valid assignment (e.g., VAR=value)
                            assignments.push(self.parse_assignment()?);
                        } else {
                            // Invalid name before '=' is treated as a regular argument/path.
                            // FIX: Use new helper function to consolidate path/equal components
                            if is_path_start || is_potential_assignment {
                                argv.push(self.parse_word_or_path_with_equal()?);
                            } else {
                                argv.push(self.parse_word()?);
                            }
                        }
                    } else if is_path_start || is_potential_assignment {
                        // FIX: If Word is followed by Slash OR Equal, consolidate path/equal components
                        argv.push(self.parse_word_or_path_with_equal()?);
                    } else {
                        // Regular argument or command name. Use parse_word.
                        argv.push(self.parse_word()?);
                    }
                }
                
                // When a Slash is seen, it must be the start of an absolute path, so we call the consolidation logic.
                Token::Slash => {
                    argv.push(self.parse_word_or_path_with_equal()?);
                }
                
                Token::RedirectLeft | Token::RedirectRight => {
                    // Logic for redirects
                    match self.peek() {
                        Some(Token::RedirectRight) if matches!(self.peek_n(1), Some(Token::RedirectRight)) => {
                            self.consume(); // consume first '>'
                            self.consume(); // consume second '>'
                            let target = self.parse_word()?;
                            redirects.push(AstNode::Redirect {
                                kind: RedirectKind::Append,
                                target,
                            });
                        }
                        _ => {
                            redirects.push(self.parse_redirect()?);
                        }
                    }
                }
                _ => break,
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

    /// Tries to parse a path (Word/Slash sequence) OR consolidate a Word = Word sequence.
    /// Used for command arguments where "=" and "/" should be part of the word.
    fn parse_word_or_path_with_equal(&mut self) -> Result<Word, ParsingError> {
        let mut path_parts = Vec::new();

        while let Some(token) = self.peek() {
            match token {
                // Collect path components (Word or Slash)
                Token::Word(parts) => {
                    path_parts.extend(parts.clone());
                    self.consume();
                }
                Token::Slash => {
                    path_parts.push(WordPart::Literal("/".to_string()));
                    self.consume();
                }
                
                // FIX: Consolidate "Equal" followed by a Word into the current word if we are still building it.
                Token::Equal => {
                    if path_parts.is_empty() {
                        // If "=" is the first token, we treat it as an unexpected token.
                        return Err(ParsingError::UnexpectedToken(self.consume().unwrap()));
                    }
                    // Consume '='
                    self.consume();
                    path_parts.push(WordPart::Literal("=".to_string()));

                    // Expect a value (Word) immediately after '='
                    if let Some(Token::Word(value_parts)) = self.peek() {
                        path_parts.extend(value_parts.clone());
                        self.consume();
                        // Stop processing after consolidating NAME=VALUE argument
                        break; 
                    } else {
                        // If '=' is not followed by a word (e.g., it's at the end: cmd arg=), stop here.
                        break; 
                    }
                }
                // Stop if we hit any other separator
                _ => break,
            }
        }

        if path_parts.is_empty() {
            return Err(ParsingError::ExpectedWord);
        }

        // Return Literal if only one simple part, otherwise Compound.
        if path_parts.len() == 1 {
            if let WordPart::Literal(s) = &path_parts[0] {
                return Ok(Word::Literal(s.clone()));
            }
        }

        Ok(Word::Compound(path_parts))
    }


    /// This is the original simple parse_path, now replaced by parse_word_or_path_with_equal for argv.
    /// Kept for backward compatibility but might not be used if parse_word_or_path_with_equal is used everywhere.
    fn parse_path(&mut self) -> Result<Word, ParsingError> {
        self.parse_word_or_path_with_equal()
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

        // Validate name (The initial character check is now in parse_command)
        if name.is_empty() {
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
        // Use parse_word here since redirect targets are usually single words/paths that don't need re-parsing
        let target = self.parse_word()?; 

        Ok(AstNode::Redirect { kind, target })
    }

    /// Parse a word from the current token (used primarily by parse_redirect and parse_assignment)
    fn parse_word(&mut self) -> Result<Word, ParsingError> {
        match self.consume() {
            Some(Token::Word(parts)) => Self::word_parts_to_ast_word(parts),
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
                    return Err(ParsingError::UnsupportedSubstitution);
                }
                WordPart::ParamSubst(content) => {
                    // For now, we'll just return an error for parameter substitutions
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
    // Assuming split_into_tokens exists in crate::lexer

    fn lit(s: &str) -> Word {
        Word::Literal(s.to_string())
    }
    
    // Helper to check if the Compound word correctly represents the string
    fn assert_compound_eq(word: &Word, expected: &str) {
        if let Word::Compound(parts) = word {
            let actual: String = parts.iter().map(|p| match p {
                WordPart::Literal(s) => s.clone(),
                _ => panic!("Expected only Literal parts in compound for this test"),
            }).collect();
            assert_eq!(actual, expected);
        } else {
            panic!("Expected Word::Compound, got {:?}", word);
        }
    }

    #[test]
    fn test_cmake_args_consolidation_fix() {
        // Tokens for "cmake .. -DCMAKE_BUILD_TYPE=Release"
        let tokens = vec![
            Token::Word(vec![WordPart::Literal("cmake".to_string())]),
            Token::Word(vec![WordPart::Literal("..".to_string())]),
            Token::Word(vec![WordPart::Literal("-DCMAKE_BUILD_TYPE".to_string())]),
            Token::Equal,
            Token::Word(vec![WordPart::Literal("Release".to_string())]),
        ];
        
        let ast = construct_ast(tokens).unwrap();

        if let AstNode::Command { argv, .. } = ast {
            assert_eq!(argv.len(), 3, "Should have 3 arguments: cmake, .., and -D...");
            assert_eq!(argv[0], lit("cmake"));
            assert_eq!(argv[1], lit(".."));

            // The flag should be consolidated into one argument
            assert_compound_eq(&argv[2], "-DCMAKE_BUILD_TYPE=Release");
        } else {
            panic!("Expected Command node");
        }
    }
    
    #[test]
    fn test_path_argument_fix_cd_parent() {
        let tokens = vec![
            Token::Word(vec![WordPart::Literal("cd".to_string())]),
            Token::Word(vec![WordPart::Literal("..".to_string())]),
            Token::Slash,
            Token::Word(vec![WordPart::Literal("..".to_string())]),
        ];
        let ast = construct_ast(tokens).unwrap();

        if let AstNode::Command { argv, .. } = ast {
            assert_eq!(argv.len(), 2, "Command should have 2 arguments: 'cd' and '.. / ..'");
            assert_eq!(argv[0], lit("cd"), "First argument must be the command 'cd'");
            
            assert_compound_eq(&argv[1], "../..");
        } else {
            panic!("Expected Command node");
        }
    }
}