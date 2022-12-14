#![deny(
    clippy::semicolon_if_nothing_returned,
    clippy::debug_assert_with_mut_call,
    clippy::float_arithmetic
)]
#![warn(clippy::cargo, clippy::pedantic, clippy::undocumented_unsafe_blocks)]
#![allow(
    clippy::cast_lossless,
    clippy::enum_glob_use,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::unreadable_literal,
    clippy::wildcard_imports,
    clippy::wildcard_dependencies,
    clippy::similar_names,
    clippy::bool_to_int_with_if,
    dead_code
)]

use ariadne::Report;
use lexer::TokenKind;

// pub mod ssa;
pub mod defs;
pub mod lexer;
pub mod parser;
pub mod strings;
pub mod types;

#[cfg(test)]
mod tests;

pub type Span = logos::Span;

#[derive(Debug, Clone)]
pub enum ErrorKind {
    General(String),

    Unexpected {
        expected: Vec<TokenKind>,
        found: Option<TokenKind>,
    },

    UnclosedDelimiter {
        delimiter: TokenKind,
        delimiter_span: Span,
        expected: TokenKind,
        found: Option<TokenKind>,
    },

    UndeclaredVar {
        var_name: String,
    },

    NoTle,
}

#[derive(Debug, Clone)]
pub struct Error {
    span: Span,
    kind: Box<ErrorKind>,
    label: Option<&'static str>,
}

impl Error {
    pub fn general(span: Span, msg: &str, label: Option<&'static str>) -> Self {
        Self {
            span,
            kind: Box::new(ErrorKind::General(msg.to_owned())),
            label,
        }
    }

    pub fn unexpected(
        span: Span,
        expected: Vec<TokenKind>,
        found: Option<TokenKind>,
        label: Option<&'static str>,
    ) -> Self {
        Self {
            span,
            kind: Box::new(ErrorKind::Unexpected { expected, found }),
            label,
        }
    }

    pub fn undeclared_var(span: Span, var_name: &str, label: Option<&'static str>) -> Self {
        Self {
            span,
            kind: Box::new(ErrorKind::UndeclaredVar {
                var_name: var_name.to_owned(),
            }),
            label,
        }
    }

    pub fn no_top_level_expr() -> Self {
        Self {
            span: 0..0,
            kind: Box::new(ErrorKind::NoTle),
            label: None,
        }
    }

    pub fn span(&self) -> &Span {
        &self.span
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn label(&self) -> Option<&'static str> {
        self.label
    }

    fn label_msg(&self, msg: &str) -> String {
        match self.label() {
            Some(label) => format!("[{label}] {msg}"),
            None => msg.to_owned(),
        }
    }

    pub fn generate_report(&self) -> Report {
        use ariadne::*;

        match self.kind() {
            ErrorKind::General(msg) => Report::build(ReportKind::Error, (), 8)
                .with_message(msg)
                .with_label(Label::new(self.span().clone()))
                .finish(),

            ErrorKind::Unexpected { expected, found } => {
                let mut msg = String::new();
                if let Some(label) = self.label() {
                    msg.push_str(format!("[{label}] ").as_str());
                }

                let mut msg = self.label_msg("unexpected input");
                if let Some(found) = found {
                    msg.push_str(format!(", found '{found}'").as_str());
                }

                let mut report = Report::build(ReportKind::Error, (), 8)
                    .with_message(msg)
                    .with_label(
                        Label::new(self.span().clone())
                            .with_message("compiler did not expect this")
                            .with_color(Color::Default),
                    );

                match expected.len() {
                    1 => report = report.with_note(format!("expected '{}'", expected[0])),
                    len if len > 1 => {
                        report = report.with_note(format!(
                            "expected one of {}",
                            expected
                                .iter()
                                .map(|t| format!("'{t}'"))
                                .collect::<Vec<String>>()
                                .join(", ")
                        ));
                    }

                    _ => {}
                }

                report.finish()
            }

            ErrorKind::UnclosedDelimiter {
                delimiter,
                delimiter_span: _,
                expected,
                found: _,
            } => Report::build(ReportKind::Error, (), 8)
                .with_message("unclosed delimiter")
                .with_label(
                    Label::new(self.span().clone())
                        .with_message("expected delimiter for this block")
                        .with_color(Color::Default),
                )
                .with_help(format!(
                    "try inserting {} at the end of the {}",
                    expected.fg(Color::Green),
                    match delimiter {
                        TokenKind::ArrayOpen => "array declaration",
                        TokenKind::GroupOpen => "grouping",
                        _ => "code block",
                    }
                ))
                .finish(),

            ErrorKind::UndeclaredVar { var_name } => Report::build(ReportKind::Error, (), 8)
                .with_message(format!("use of undeclared variable `{var_name}`"))
                .with_label(Label::new(self.span().clone()))
                .finish(),

            ErrorKind::NoTle => Report::build(ReportKind::Error, (), 8)
                .with_message("script has no top-level expression")
                .finish(),
        }
    }
}

impl chumsky::Error<TokenKind> for Error {
    type Span = crate::Span;
    type Label = &'static str;

    fn expected_input_found<Iter: IntoIterator<Item = Option<TokenKind>>>(
        span: Self::Span,
        expected: Iter,
        found: Option<TokenKind>,
    ) -> Self {
        Self {
            span,
            kind: Box::new(ErrorKind::Unexpected {
                expected: expected.into_iter().flatten().collect(),
                found,
            }),
            label: None,
        }
    }

    fn unclosed_delimiter(
        unclosed_span: Self::Span,
        delimiter: TokenKind,
        span: Self::Span,
        expected: TokenKind,
        found: Option<TokenKind>,
    ) -> Self {
        Self {
            span,
            kind: Box::new(ErrorKind::UnclosedDelimiter {
                delimiter,
                delimiter_span: unclosed_span,
                expected,
                found,
            }),
            label: None,
        }
    }

    fn with_label(self, label: Self::Label) -> Self {
        Self {
            span: self.span,
            kind: self.kind,
            label: Some(label),
        }
    }

    fn merge(self, _other: Self) -> Self {
        // FIXME: Actually merge the errors?
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Exp,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shr,
    Shl,

    BitXor,
    BitAnd,
    BitOr,

    Eq,
    NotEq,

    Greater,
    GreaterEq,
    Less,
    LessEq,

    Or,
    Xor,
    And,

    Clow,
    Cerm,

    Assign,
}

impl Operator {
    #[inline]
    pub const fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Self::Exp
                | Self::Add
                | Self::Sub
                | Self::Mul
                | Self::Div
                | Self::Rem
                | Self::Shr
                | Self::Shl
                | Self::BitXor
                | Self::BitAnd
                | Self::BitOr
        )
    }

    #[inline]
    pub const fn is_boolean(&self) -> bool {
        matches!(
            self,
            Self::Eq | Self::NotEq | Self::Greater | Self::GreaterEq | Self::Less | Self::LessEq
        )
    }

    #[inline]
    pub const fn is_logical(&self) -> bool {
        matches!(
            self,
            Self::Eq
                | Self::NotEq
                | Self::Greater
                | Self::GreaterEq
                | Self::Less
                | Self::LessEq
                | Self::Or
                | Self::Xor
                | Self::And
        )
    }
}

#[macro_export]
macro_rules! interned {
    ($string:expr) => {{
        $crate::strings::intern_str($string)
    }};
}
