// Lexer (tokenizer) for the Alpha programming language.
use crate::diagnostics::error::{AlphaError, Span};
use crate::frontend::token::{Token, TokenKind};

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    cursor: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let chars: Vec<(usize, char)> = source.char_indices().collect();
        Self {
            source,
            chars,
            cursor: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenize the entire source string into tokens.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, AlphaError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.is_at_end() {
                let span = Span::new(self.line, self.col, self.current_byte_pos(), self.current_byte_pos());
                tokens.push(Token::new(TokenKind::Eof, span, ""));
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).map(|&(_, c)| c)
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.cursor + 1).map(|&(_, c)| c)
    }

    fn current_byte_pos(&self) -> usize {
        self.chars
            .get(self.cursor)
            .map(|&(pos, _)| pos)
            .unwrap_or(self.source.len())
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.chars.len()
    }

    fn advance(&mut self) -> Option<char> {
        if self.is_at_end() {
            return None;
        }
        let (_, c) = self.chars[self.cursor];
        self.cursor += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '/' && self.peek_next() == Some('/') {
                // Single-line comment, consume until newline or EOF
                self.advance(); // consume first '/'
                self.advance(); // consume second '/'
                while let Some(nc) = self.peek() {
                    if nc == '\n' {
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, AlphaError> {
        let start_line = self.line;
        let start_col = self.col;
        let start_pos = self.current_byte_pos();

        let ch = match self.advance() {
            Some(c) => c,
            None => {
                let span = Span::new(start_line, start_col, start_pos, start_pos);
                return Ok(Token::new(TokenKind::Eof, span, ""));
            }
        };

        // Two-character and one-character tokens
        let kind = match ch {
            '+' => {
                if self.peek() == Some('+') {
                    self.advance();
                    TokenKind::PlusPlus
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if self.peek() == Some('-') {
                    self.advance();
                    TokenKind::MinusMinus
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::MinusEqual
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PercentEqual
                } else {
                    TokenKind::Percent
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::EqualEqual
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::FatArrow
                } else {
                    TokenKind::Equal
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::BangEqual
                } else if self.peek() == Some('?') {
                    self.advance();
                    TokenKind::BangQuestion
                } else {
                    TokenKind::Bang
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::AndAnd
                } else {
                    let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex("Unexpected character '&'. Did you mean '&&'?", span));
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::OrOr
                } else {
                    let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex("Unexpected character '|'. Did you mean '||'?", span));
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }
            '.' => {
                if self.peek() == Some('.') {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        TokenKind::DotDotDot
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,

            '#' => {
                if self.peek() == Some('(') {
                    self.advance(); // consume '('
                    let mut hex_content = String::new();
                    while let Some(c) = self.peek() {
                        if c == ')' {
                            self.advance(); // consume ')'
                            break;
                        } else if c == '\n' || c == '\r' {
                            let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                            return Err(AlphaError::lex("Unclosed hexcolor literal '#(...)' before newline", span));
                        } else {
                            hex_content.push(c);
                            self.advance();
                        }
                    }
                    let cleaned = hex_content.trim().trim_start_matches('#').to_string();
                    let end_pos = self.current_byte_pos();
                    let span = Span::new(start_line, start_col, start_pos, end_pos);
                    return Ok(Token::new(
                        TokenKind::HexColorLit(cleaned),
                        span,
                        &self.source[start_pos..end_pos],
                    ));
                } else {
                    let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex("Unexpected character '#'. Hex colors must be written as '#(HEX)' e.g. '#(FFF00)'", span));
                }
            }

            '$' => {
                if self.peek() == Some('"') {
                    self.advance(); // consume opening quote
                    if self.peek() == Some('"') && self.peek_next() == Some('"') {
                        self.advance(); // quote 2
                        self.advance(); // quote 3
                        return self.lex_multiline_string(true, start_line, start_col, start_pos);
                    } else {
                        return self.lex_interpolated_string(start_line, start_col, start_pos);
                    }
                } else {
                    let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex("Unexpected character '$'. Did you mean '$\"string\"'?", span));
                }
            }

            // String literal
            '"' => {
                if self.peek() == Some('"') && self.peek_next() == Some('"') {
                    self.advance(); // quote 2
                    self.advance(); // quote 3
                    return self.lex_multiline_string(false, start_line, start_col, start_pos);
                } else {
                    return self.lex_string(start_line, start_col, start_pos);
                }
            }

            // Character literal
            '\'' => return self.lex_char(start_line, start_col, start_pos),

            // Numbers
            '0'..='9' => return self.lex_number(ch, start_line, start_col, start_pos),

            '@' => {
                let mut dir = String::new();
                while let Some(c) = self.peek() {
                    if c.is_alphabetic() || c == '_' {
                        dir.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if dir == "strict" {
                    let end_pos = self.current_byte_pos();
                    let span = Span::new(start_line, start_col, start_pos, end_pos);
                    return Ok(Token::new(TokenKind::AtStrict, span, "@strict"));
                } else {
                    let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex(format!("Unknown directive '@{}'. Did you mean '@strict'?", dir), span));
                }
            }

            // Identifiers / Keywords
            c if is_ident_start(c) => {
                return self.lex_identifier(c, start_line, start_col, start_pos)
            }

            other => {
                let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                return Err(AlphaError::lex(
                    format!("Unexpected character: '{}'", other),
                    span,
                ));
            }
        };

        let end_pos = self.current_byte_pos();
        let span = Span::new(start_line, start_col, start_pos, end_pos);
        let text = self.source[start_pos..end_pos].to_string();
        Ok(Token::new(kind, span, text))
    }

    fn lex_string(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance(); // consume closing quote
                let end_pos = self.current_byte_pos();
                let span = Span::new(start_line, start_col, start_pos, end_pos);
                return Ok(Token::new(
                    TokenKind::StringLit(s),
                    span,
                    &self.source[start_pos..end_pos],
                ));
            } else if c == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('0') => s.push('\0'),
                    Some(other) => {
                        let span = Span::new(self.line, self.col, start_pos, self.current_byte_pos());
                        return Err(AlphaError::lex(
                            format!("Invalid escape sequence '\\{}'", other),
                            span,
                        ));
                    }
                    None => {
                        let span = Span::new(self.line, self.col, start_pos, self.current_byte_pos());
                        return Err(AlphaError::lex("Unterminated string escape", span));
                    }
                }
            } else {
                s.push(c);
                self.advance();
            }
        }

        let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
        Err(AlphaError::lex("Unterminated string literal", span))
    }

    fn lex_multiline_string(
        &mut self,
        is_interpolated: bool,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let mut s = String::new();
        while !self.is_at_end() {
            if self.peek() == Some('"') && self.peek_next() == Some('"') {
                if let Some(&(_, c3)) = self.chars.get(self.cursor + 2) {
                    if c3 == '"' {
                        self.advance(); // quote 1
                        self.advance(); // quote 2
                        self.advance(); // quote 3
                        let end_pos = self.current_byte_pos();
                        let span = Span::new(start_line, start_col, start_pos, end_pos);
                        let kind = if is_interpolated {
                            TokenKind::InterpolatedMultilineString(s)
                        } else {
                            TokenKind::MultilineString(s)
                        };
                        return Ok(Token::new(kind, span, &self.source[start_pos..end_pos]));
                    }
                }
            }

            if let Some(c) = self.advance() {
                s.push(c);
            }
        }

        let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
        Err(AlphaError::lex("Unterminated multiline string literal '\"\"\"'", span))
    }

    fn lex_interpolated_string(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
                let end_pos = self.current_byte_pos();
                let span = Span::new(start_line, start_col, start_pos, end_pos);
                return Ok(Token::new(
                    TokenKind::InterpolatedString(s),
                    span,
                    &self.source[start_pos..end_pos],
                ));
            } else if c == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('{') => s.push('{'),
                    Some('}') => s.push('}'),
                    Some('0') => s.push('\0'),
                    Some(other) => {
                        s.push('\\');
                        s.push(other);
                    }
                    None => {
                        let span = Span::new(self.line, self.col, start_pos, self.current_byte_pos());
                        return Err(AlphaError::lex("Unterminated string escape", span));
                    }
                }
            } else {
                s.push(c);
                self.advance();
            }
        }

        let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
        Err(AlphaError::lex("Unterminated interpolated string literal", span))
    }

    fn lex_char(
        &mut self,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let c = match self.advance() {
            Some('\\') => match self.advance() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('\\') => '\\',
                Some('\'') => '\'',
                Some('0') => '\0',
                Some(other) => {
                    let span = Span::new(self.line, self.col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex(
                        format!("Invalid char escape '\\{}'", other),
                        span,
                    ));
                }
                None => {
                    let span = Span::new(self.line, self.col, start_pos, self.current_byte_pos());
                    return Err(AlphaError::lex("Unterminated char escape", span));
                }
            },
            Some('\'') => {
                let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                return Err(AlphaError::lex("Empty char literal", span));
            }
            Some(ch) => ch,
            None => {
                let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
                return Err(AlphaError::lex("Unterminated char literal", span));
            }
        };

        if self.advance() != Some('\'') {
            let span = Span::new(start_line, start_col, start_pos, self.current_byte_pos());
            return Err(AlphaError::lex("Expected closing quote for char literal", span));
        }

        let end_pos = self.current_byte_pos();
        let span = Span::new(start_line, start_col, start_pos, end_pos);
        Ok(Token::new(
            TokenKind::CharLit(c),
            span,
            &self.source[start_pos..end_pos],
        ))
    }

    fn lex_number(
        &mut self,
        first: char,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let mut text = String::new();
        text.push(first);
        let mut is_float = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '_' {
                if c != '_' {
                    text.push(c);
                }
                self.advance();
            } else if c == '.' && !is_float && self.peek_next().map_or(false, |n| n.is_ascii_digit()) {
                is_float = true;
                text.push('.');
                self.advance(); // consume '.'
            } else {
                break;
            }
        }

        let end_pos = self.current_byte_pos();
        let span = Span::new(start_line, start_col, start_pos, end_pos);

        if is_float {
            match text.parse::<f64>() {
                Ok(val) => Ok(Token::new(TokenKind::FloatLit(val), span, text)),
                Err(_) => Err(AlphaError::lex(format!("Invalid float literal '{}'", text), span)),
            }
        } else {
            match text.parse::<i64>() {
                Ok(val) => Ok(Token::new(TokenKind::IntLit(val), span, text)),
                Err(_) => Err(AlphaError::lex(format!("Invalid integer literal '{}'", text), span)),
            }
        }
    }

    fn lex_identifier(
        &mut self,
        first: char,
        start_line: usize,
        start_col: usize,
        start_pos: usize,
    ) -> Result<Token, AlphaError> {
        let mut text = String::new();
        text.push(first);

        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                text.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let end_pos = self.current_byte_pos();
        let span = Span::new(start_line, start_col, start_pos, end_pos);

        let kind = match text.as_str() {
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "function" => TokenKind::Function,
            "public" => TokenKind::Public,
            "here" => TokenKind::Here,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "elseif" => TokenKind::ElseIf,
            "else" => TokenKind::Else,
            "not" => TokenKind::Not,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "for" => TokenKind::For,
            "each" => TokenKind::Each,
            "in" => TokenKind::In,
            "import" => TokenKind::Import,
            "web" => TokenKind::Web,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "check" => TokenKind::Check,
            "structure" => TokenKind::Structure,
            "attach" => TokenKind::Attach,
            "static" => TokenKind::Static,
            "nonstatic" => TokenKind::Nonstatic,
            "this" => TokenKind::This,
            "make" => TokenKind::Make,
            "schedule" => TokenKind::Schedule,
            "verify" => TokenKind::Verify,
            "system" => TokenKind::System,
            "enum" => TokenKind::Enum,
            "module" => TokenKind::Module,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nothing" => TokenKind::Nothing,
            "int" => TokenKind::TypeInt,
            "unt" => TokenKind::TypeUnt,
            "float" => TokenKind::TypeFloat,
            "string" => TokenKind::TypeString,
            "char" => TokenKind::TypeChar,
            "boolean" => TokenKind::TypeBoolean,
            "hexcolor" => TokenKind::TypeHexColor,
            "something" => TokenKind::TypeSomething,
            "everything" => TokenKind::TypeEverything,
            "vect" => TokenKind::TypeVect,
            "map" => TokenKind::TypeMap,
            _ => TokenKind::Ident(text.clone()),
        };

        Ok(Token::new(kind, span, text))
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '?'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '?'
}
