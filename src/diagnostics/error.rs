#![allow(dead_code)]
// Error handling and diagnostic reporting for the Alpha language.
use std::fmt;

/// Source code location span (line, column, byte offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(line: usize, col: usize, start: usize, end: usize) -> Self {
        Self { line, col, start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            line: self.line.min(other.line),
            col: if self.start <= other.start { self.col } else { other.col },
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// Category of error that occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Compile,
    Type,
    Runtime,
    Io,
}

impl ErrorKind {
    pub fn title(&self) -> &'static str {
        match self {
            ErrorKind::Compile => "Compile Error",
            ErrorKind::Type => "Compile Error (Type)",
            ErrorKind::Runtime => "Runtime Error",
            ErrorKind::Io => "IO Error",
        }
    }
}

/// An error with source position, description, and helpful hint.
#[derive(Debug, Clone)]
pub struct AlphaError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Span,
    pub file: Option<String>,
    pub hint: Option<String>,
}

impl AlphaError {
    pub fn compile(msg: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::Compile,
            message: msg.into(),
            span,
            file: None,
            hint: None,
        }
    }

    pub fn lex(msg: impl Into<String>, span: Span) -> Self {
        Self::compile(msg, span)
    }

    pub fn parse(msg: impl Into<String>, span: Span) -> Self {
        Self::compile(msg, span)
    }

    pub fn type_error(msg: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::Type,
            message: msg.into(),
            span,
            file: None,
            hint: None,
        }
    }

    pub fn runtime(msg: impl Into<String>, span: Span) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: msg.into(),
            span,
            file: None,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Format error with source code snippet, line pointers, and helpful hints.
    pub fn format_with_source(&self, source: &str) -> String {
        let file_name = self.file.as_deref().unwrap_or("<source>");
        let mut out = format!("{}: {}\n", self.kind.title(), self.message);

        if self.span.line > 0 {
            out.push_str(&format!("  --> {}:{}:{}\n", file_name, self.span.line, self.span.col));
            out.push_str("   |\n");

            let lines: Vec<&str> = source.lines().collect();
            if self.span.line <= lines.len() {
                let line_idx = self.span.line - 1;
                let line_num = self.span.line;
                let line_content = lines[line_idx];

                out.push_str(&format!(" {:3} | {}\n", line_num, line_content));
                let caret_pad = " ".repeat(self.span.col.saturating_sub(1));
                let width = (self.span.end.saturating_sub(self.span.start)).max(1);
                let carets = "^".repeat(width);
                out.push_str(&format!("     | {}{}\n", caret_pad, carets));
            }
        } else {
            out.push_str(&format!("  --> {}\n", file_name));
        }

        if let Some(hint) = &self.hint {
            out.push_str(&format!("   = Hint: {}\n", hint));
        }

        out
    }
}

impl fmt::Display for AlphaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file_name = self.file.as_deref().unwrap_or("<source>");
        if self.span.line > 0 {
            write!(
                f,
                "{}: {} (at {}:{}:{})",
                self.kind.title(),
                self.message,
                file_name,
                self.span.line,
                self.span.col
            )?;
        } else {
            write!(f, "{}: {}", self.kind.title(), self.message)?;
        }
        if let Some(hint) = &self.hint {
            write!(f, "\n   = Hint: {}", hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for AlphaError {}
