use oxc_ast::{CommentKind, Span, Trivias};

#[derive(Debug, Default)]
pub struct TriviaBuilder {
    trivias: Trivias,
}

impl TriviaBuilder {
    #[allow(clippy::missing_const_for_fn)]
    pub fn build(self) -> Trivias {
        self.trivias
    }

    pub fn add_single_line_comment(&mut self, start: u32, end: u32, is_config_comment: bool) {
        // skip leading `//`
        self.trivias.add_comment(
            Span::new(start + 2, end),
            if is_config_comment {
                CommentKind::ConfigurationSingleLine
            } else {
                CommentKind::SingleLine
            },
        );
    }

    #[allow(unused)]
    pub fn add_multi_line_comment(&mut self, start: u32, end: u32) {
        // skip leading `/*` and trailing */
        self.trivias.add_comment(Span::new(start + 2, end - 2), CommentKind::MultiLine);
    }
}

pub fn is_eslint_configuration_comment(comment: &str) -> bool {
    // TODO: extend to support other types of configuration comments
    comment.starts_with("eslint-disable-next-line")
}
