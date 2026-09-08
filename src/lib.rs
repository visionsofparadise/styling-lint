//! Configurable, insertion-only spacing between Rust statement groups.
//!
//! The engine visits parsed Rust blocks. Macro token bodies remain opaque, and
//! adjacent statements on the same physical line retain their original layout.

use std::collections::BTreeMap;

use proc_macro2::LineColumn;
use serde::Deserialize;
use syn::{Expr, Stmt, spanned::Spanned, visit::Visit};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
/// Controls the minimum spacing and which statement kinds form compact groups.
pub struct SpacingOptions {
    /// Required blank lines between groups; defaults to one. Zero disables findings.
    pub min_blank_lines: usize,
    /// Allows consecutive declarations and assignments without a blank line.
    pub group_assignments: bool,
    /// Allows consecutive call statements without a blank line.
    pub group_calls: bool,
}

impl Default for SpacingOptions {
    fn default() -> Self {
        Self {
            min_blank_lines: 1,
            group_assignments: true,
            group_calls: true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// A statement boundary with insufficient blank lines in the original source.
pub struct Diagnostic {
    /// One-based line of the following statement, including its attributes.
    pub line: usize,
    /// One-based Unicode character column of the following statement.
    pub column: usize,
    /// Configured minimum blank lines.
    pub required: usize,
    /// Existing whitespace-only lines outside comments between the statements.
    pub found: usize,
}

#[derive(Debug)]
/// Source with missing line breaks inserted, accompanied by original diagnostics.
pub struct Outcome {
    /// Original source bytes plus inserted line breaks.
    pub output: String,
    /// Findings in source order. An empty list means no spacing changes are needed.
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(PartialEq, Eq)]
enum Group {
    Assignment,
    Call,
    Other,
}

fn group(statement: &Stmt) -> Group {
    match statement {
        Stmt::Local(_) => Group::Assignment,
        Stmt::Expr(expression, semicolon) => match expression {
            Expr::Assign(_) => Group::Assignment,
            Expr::Binary(binary)
                if matches!(
                    binary.op,
                    syn::BinOp::AddAssign(_)
                        | syn::BinOp::SubAssign(_)
                        | syn::BinOp::MulAssign(_)
                        | syn::BinOp::DivAssign(_)
                        | syn::BinOp::RemAssign(_)
                        | syn::BinOp::BitXorAssign(_)
                        | syn::BinOp::BitAndAssign(_)
                        | syn::BinOp::BitOrAssign(_)
                        | syn::BinOp::ShlAssign(_)
                        | syn::BinOp::ShrAssign(_)
                ) =>
            {
                Group::Assignment
            }
            expression if semicolon.is_some() && is_call(expression) => Group::Call,
            _ => Group::Other,
        },
        Stmt::Macro(_) => Group::Call,
        Stmt::Item(_) => Group::Other,
    }
}

fn is_call(expression: &Expr) -> bool {
    match expression {
        Expr::Call(_) | Expr::MethodCall(_) | Expr::Macro(_) => true,
        Expr::Try(expression) => is_call(&expression.expr),
        Expr::Await(expression) => is_call(&expression.base),
        Expr::Paren(expression) => is_call(&expression.expr),
        _ => false,
    }
}

struct Gap {
    blank_lines: usize,
    insertion: usize,
    needs_newline: bool,
}

fn inspect_gap(gap: &str) -> Gap {
    let bytes = gap.as_bytes();
    let mut offset = 0;
    let mut depth = 0;
    let mut line_comment = false;
    let mut content = true;
    let mut blank_lines = 0;
    let mut insertion = None;
    let mut last_comment_end = 0;

    while offset < bytes.len() {
        if bytes[offset] == b'\n' {
            if depth == 0 {
                if !content {
                    blank_lines += 1;
                }

                insertion.get_or_insert(offset + 1);
            }

            line_comment = false;
            content = depth != 0;
            offset += 1;
        } else if line_comment {
            offset += 1;
        } else if bytes[offset..].starts_with(b"/*") {
            depth += 1;
            content = true;
            offset += 2;
        } else if depth != 0 && bytes[offset..].starts_with(b"*/") {
            depth -= 1;
            offset += 2;

            if depth == 0 {
                last_comment_end = offset;
            }
        } else if depth == 0 && bytes[offset..].starts_with(b"//") {
            line_comment = true;
            content = true;
            offset += 2;
        } else {
            if !matches!(bytes[offset], b' ' | b'\t' | b'\r' | 0x0b | 0x0c) {
                content = true;
            }

            offset += 1;
        }
    }

    Gap {
        blank_lines,
        insertion: insertion.unwrap_or(last_comment_end),
        needs_newline: insertion.is_none(),
    }
}

struct Spacing<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
    options: &'a SpacingOptions,
    insertions: BTreeMap<usize, (usize, Diagnostic)>,
}

impl Spacing<'_> {
    fn offset(&self, position: LineColumn) -> usize {
        let start = self.line_starts[position.line - 1];

        start
            + self.source[start..]
                .char_indices()
                .nth(position.column)
                .map_or(self.source.len() - start, |(offset, _)| offset)
    }
}

impl<'ast> Visit<'ast> for Spacing<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        for pair in block.stmts.windows(2) {
            let previous_group = group(&pair[0]);
            let next_group = group(&pair[1]);
            let grouped = previous_group == next_group
                && match previous_group {
                    Group::Assignment => self.options.group_assignments,
                    Group::Call => self.options.group_calls,
                    Group::Other => false,
                };
            let previous = pair[0].span().end();
            let next = pair[1].span().start();

            if grouped || next.line <= previous.line {
                continue;
            }

            let start = self.offset(previous);
            let end = self.offset(next);
            let gap = inspect_gap(&self.source[start..end]);

            if gap.blank_lines >= self.options.min_blank_lines {
                continue;
            }

            self.insertions.insert(
                start + gap.insertion,
                (
                    self.options.min_blank_lines - gap.blank_lines + usize::from(gap.needs_newline),
                    Diagnostic {
                        line: next.line,
                        column: next.column + 1,
                        required: self.options.min_blank_lines,
                        found: gap.blank_lines,
                    },
                ),
            );
        }

        syn::visit::visit_block(self, block);
    }
}

/// Parses Rust and inserts missing blank lines between statement groups.
///
/// Comments, literals, existing whitespace, and the final newline are preserved.
/// Inserted lines use the local newline style. Blank lines within comments do
/// not satisfy the minimum. Runs of assignments and calls are compact by default;
/// tail call expressions form a separate group, while statement macros retain call
/// grouping. Existing blank lines are never removed, including inside compact groups.
///
/// Returns an error for invalid Rust syntax or a minimum greater than ten. A zero
/// minimum produces no findings while still validating Rust syntax.
pub fn lint(source: &str, options: &SpacingOptions) -> Result<Outcome, syn::Error> {
    if options.min_blank_lines > 10 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "min-blank-lines must be at most 10",
        ));
    }

    let body = source.strip_prefix('\u{feff}').unwrap_or(source);
    let prefix_length = source.len() - body.len();
    let syntax = syn::parse_file(body)?;
    let mut line_starts = vec![0];

    line_starts.extend(body.match_indices('\n').map(|(offset, _)| offset + 1));

    let mut spacing = Spacing {
        source: body,
        line_starts,
        options,
        insertions: BTreeMap::new(),
    };

    spacing.visit_file(&syntax);

    let mut output = String::with_capacity(source.len());
    let mut previous = 0;
    let mut diagnostics = Vec::with_capacity(spacing.insertions.len());

    for (offset, (count, diagnostic)) in spacing.insertions {
        let offset = prefix_length + offset;

        output.push_str(&source[previous..offset]);

        let newline = if source[..offset].ends_with("\r\n")
            || (!source[..offset].ends_with('\n')
                && source[offset..]
                    .find('\n')
                    .is_some_and(|next| next > 0 && source.as_bytes()[offset + next - 1] == b'\r'))
        {
            "\r\n"
        } else {
            "\n"
        };

        for _ in 0..count {
            output.push_str(newline);
        }

        previous = offset;

        diagnostics.push(diagnostic);
    }

    output.push_str(&source[previous..]);

    Ok(Outcome {
        output,
        diagnostics,
    })
}
