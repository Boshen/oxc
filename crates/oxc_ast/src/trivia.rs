//! Trivia (called that because it's trivial) represent the parts of the source text that are largely insignificant for normal understanding of the code.
//! For example; whitespace, comments, and even conflict markers.

use std::collections::{BTreeMap, BTreeSet};

use crate::Span;

#[derive(Debug, Default)]
pub struct Trivias {
    /// Keyed by span.start
    comments: BTreeMap<u32, Comment>,

    configuration_comments: BTreeMap<u32, Comment>,
}

#[derive(Debug, Clone, Copy)]
#[allow(unused)]
pub struct Comment {
    kind: CommentKind,
    end: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum CommentKind {
    ConfigurationSingleLine,
    SingleLine,
    MultiLine,
}

impl Comment {
    #[must_use]
    pub fn new(end: u32, kind: CommentKind) -> Self {
        Self { kind, end }
    }
}

impl Trivias {
    #[must_use]
    pub fn has_comments_between(&self, span: Span) -> bool {
        self.comments.range(span.start..span.end).count() > 0
            || self.configuration_comments.range(span.start..span.end).count() > 0
    }

    pub fn add_comment(&mut self, span: Span, kind: CommentKind) {
        let comment = Comment::new(span.end, kind);
        match kind {
            CommentKind::ConfigurationSingleLine => {
                self.configuration_comments.insert(span.start, comment);
            }
            _ => {
                self.comments.insert(span.start, comment);
            }
        }
    }

    /// Checks the stored configuration comments against the associated source text to derive
    /// the lines affected by the configuration comments.
    #[must_use]
    pub fn configuration_lines(&self, source_text: &str) -> Vec<usize> {
        let mut configuration_lines = vec![];

        for (start, comment) in &self.configuration_comments {
            let lines = self.line_numbers(source_text, *start, comment.end);
            if let Some(line) = lines.last() {
                configuration_lines.push(*line + 1);
            }
        }

        configuration_lines
    }

    /// Computes the associated line numbers given source text and starts/end offsets.
    #[must_use]
    pub fn line_numbers(&self, source_text: &str, start: u32, end: u32) -> BTreeSet<usize> {
        let start_line = source_text[..start as usize].lines().count();
        let end_line = source_text[..end as usize].lines().count();
        (start_line..=end_line).collect()
    }
}
