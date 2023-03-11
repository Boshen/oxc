//! An Ecma-262 Lexer / Tokenizer
//! Prior Arts:
//!     * [jsparagus](https://github.com/mozilla-spidermonkey/jsparagus/blob/master/crates/parser/src)
//!     * [rome](https://github.com/rome/tools/tree/main/crates/rome_js_parser/src/lexer)
//!     * [rustc](https://github.com/rust-lang/rust/blob/master/compiler/rustc_lexer/src)
//!     * [v8](https://v8.dev/blog/scanner)

mod constants;
mod kind;
mod number;
mod simd;
mod string_builder;
mod token;
mod trivia_builder;

use std::{collections::VecDeque, str::Chars};

use oxc_allocator::{Allocator, String};
use oxc_ast::{ast::RegExpFlags, Atom, SourceType, Span};
use oxc_diagnostics::{Diagnostics, Error};
use simd::{SkipMultilineComment, SkipWhitespace};
pub use token::{RegExp, Token, TokenValue};

pub use self::kind::Kind;
use self::{
    constants::{
        is_identifier_part, is_identifier_start_all, is_identifier_start_ascii,
        is_irregular_line_terminator, is_irregular_whitespace, is_line_terminator, EOF,
        SINGLE_CHAR_TOKENS,
    },
    number::{parse_big_int, parse_float, parse_int},
    string_builder::AutoCow,
    trivia_builder::TriviaBuilder,
};
use crate::diagnostics;

#[derive(Debug, Clone)]
pub struct LexerCheckpoint<'a> {
    /// Remaining chars to be tokenized
    chars: Chars<'a>,

    token: Token,

    errors_pos: usize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LexerContext {
    Regular,
    /// Lex the next token, returns `<` or `{` or `JSXText`
    JsxChild,
    /// Lex the next token, returns `JsxString` or any other token
    JsxAttributeValue,
}

pub struct Lexer<'a> {
    allocator: &'a Allocator,

    source: &'a str,

    source_type: SourceType,

    current: LexerCheckpoint<'a>,

    errors: Diagnostics,

    lookahead: VecDeque<LexerCheckpoint<'a>>,

    context: LexerContext,

    pub(crate) trivia_builder: TriviaBuilder,
}

#[allow(clippy::unused_self)]
impl<'a> Lexer<'a> {
    #[must_use]
    pub fn new(
        allocator: &'a Allocator,
        source: &'a str,
        errors: Diagnostics,
        source_type: SourceType,
    ) -> Self {
        let token = Token {
            // the first token is at the start of file, so is allows on a new line
            is_on_new_line: true,
            ..Token::default()
        };
        let current =
            LexerCheckpoint { chars: source.chars(), token, errors_pos: errors.borrow().len() };
        Self {
            allocator,
            source,
            source_type,
            current,
            errors,
            lookahead: VecDeque::with_capacity(4),
            context: LexerContext::Regular,
            trivia_builder: TriviaBuilder::default(),
        }
    }

    /// Remaining string from `Chars`
    #[must_use]
    pub fn remaining(&self) -> &'a str {
        self.current.chars.as_str()
    }

    /// Creates a checkpoint storing the current lexer state.
    /// Use `rewind` to restore the lexer to the state stored in the checkpoint.
    #[must_use]
    pub fn checkpoint(&self) -> LexerCheckpoint<'a> {
        LexerCheckpoint {
            chars: self.current.chars.clone(),
            token: self.current.token.clone(),
            errors_pos: self.errors.borrow().len(),
        }
    }

    /// Rewinds the lexer to the same state as when the passed in `checkpoint` was created.
    pub fn rewind(&mut self, checkpoint: LexerCheckpoint<'a>) {
        self.errors.borrow_mut().truncate(checkpoint.errors_pos);
        self.current = checkpoint;
        self.lookahead.clear();
    }

    /// Find the nth lookahead token lazily
    pub fn lookahead(&mut self, n: u8) -> &Token {
        let n = n as usize;
        debug_assert!(n > 0);

        if self.lookahead.len() > n - 1 {
            return &self.lookahead[n - 1].token;
        }

        let checkpoint = self.checkpoint();

        if let Some(checkpoint) = self.lookahead.back() {
            self.current = checkpoint.clone();
        }

        // reset the current token for `read_next_token`,
        // otherwise it will contain the token from
        // `self.current = checkpoint`
        self.current.token = Token::default();

        for _i in self.lookahead.len()..n {
            let kind = self.read_next_token();
            let peeked = self.finish_next(kind);
            self.lookahead.push_back(LexerCheckpoint {
                chars: self.current.chars.clone(),
                token: peeked,
                errors_pos: self.errors.borrow().len(),
            });
        }

        self.current = checkpoint;

        &self.lookahead[n - 1].token
    }

    /// Set context
    pub fn set_context(&mut self, context: LexerContext) {
        self.context = context;
    }

    /// Main entry point
    pub fn next_token(&mut self) -> Token {
        if let Some(checkpoint) = self.lookahead.pop_front() {
            self.current.chars = checkpoint.chars;
            self.current.errors_pos = checkpoint.errors_pos;
            return checkpoint.token;
        }
        let kind = self.read_next_token();
        self.finish_next(kind)
    }

    fn finish_next(&mut self, kind: Kind) -> Token {
        self.current.token.kind = kind;
        self.current.token.end = self.offset();
        debug_assert!(self.current.token.start <= self.current.token.end);
        std::mem::take(&mut self.current.token)
    }

    /// Re-tokenize the current `/` or `/=` and return `RegExp`
    /// See Section 12:
    ///   The `InputElementRegExp` goal symbol is used in all syntactic grammar contexts
    ///   where a `RegularExpressionLiteral` is permitted
    /// Which meams the parser needs to re-tokenize on `PrimaryExpression`,
    /// `RegularExpressionLiteral` only appear on the right hand side of `PrimaryExpression`
    pub fn next_regex(&mut self, kind: Kind) -> Token {
        self.current.token.start = self.offset()
            - match kind {
                Kind::Slash => 1,
                Kind::SlashEq => 2,
                _ => unreachable!(),
            };
        let kind = self.read_regex(kind);
        self.lookahead.clear();
        self.finish_next(kind)
    }

    pub fn next_right_angle(&mut self) -> Token {
        let kind = self.read_right_angle();
        self.lookahead.clear();
        self.finish_next(kind)
    }

    /// Re-tokenize the current `}` token for `TemplateSubstitutionTail`
    /// See Section 12, the parser needs to re-tokenize on `TemplateSubstitutionTail`,
    pub fn next_template_substitution_tail(&mut self) -> Token {
        self.current.token.start = self.offset() - 1;
        let kind = self.read_template_literal(Kind::TemplateMiddle, Kind::TemplateTail);
        self.lookahead.clear();
        self.finish_next(kind)
    }

    /// Expand the current token for `JSXIdentifier`
    pub fn next_jsx_identifier(&mut self, prev_len: u32) -> Token {
        let kind = self.read_jsx_identifier(prev_len);
        self.lookahead.clear();
        self.finish_next(kind)
    }

    /// Re-tokenize '<<' or '<=' or '<<=' to '<'
    pub fn re_lex_as_typescript_l_angle(&mut self, kind: Kind) -> Token {
        let offset = match kind {
            Kind::ShiftLeft | Kind::LtEq => 2,
            Kind::ShiftLeftEq => 3,
            _ => unreachable!(),
        };
        self.current.token.start = self.offset() - offset;
        self.current.chars = self.source[self.current.token.start as usize + 1..].chars();
        let kind = Kind::LAngle;
        self.lookahead.clear();
        self.finish_next(kind)
    }

    /// Re-tokenize '>>' or '>=' or '>>>' or '>>=' or '>>>=' to '<'
    pub fn re_lex_as_typescript_r_angle(&mut self, kind: Kind) -> Token {
        let offset = match kind {
            Kind::ShiftRight | Kind::GtEq => 2,
            Kind::ShiftRightEq | Kind::ShiftRight3 => 3,
            Kind::ShiftRight3Eq => 4,
            _ => unreachable!(),
        };
        self.current.token.start = self.offset() - offset;
        self.current.chars = self.source[self.current.token.start as usize + 1..].chars();
        let kind = Kind::RAngle;
        self.lookahead.clear();
        self.finish_next(kind)
    }

    // ---------- Private Methods ---------- //
    fn error<T: Into<Error>>(&mut self, error: T) {
        self.errors.borrow_mut().push(error.into());
    }

    /// Get the length offset from the source, in UTF-8 bytes
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    fn offset(&self) -> u32 {
        (self.source.len() - self.current.chars.as_str().len()) as u32
    }

    /// Get the current unterminated token range
    fn unterminated_range(&self) -> Span {
        Span::new(self.current.token.start, self.offset())
    }

    /// Peek the next char without advancing the position
    #[inline]
    fn peek(&self) -> char {
        self.current.chars.clone().next().unwrap_or(EOF)
    }

    /// Peek the next next char without advancing the position
    fn peek2(&self) -> char {
        let mut chars = self.current.chars.clone();
        chars.next();
        chars.next().unwrap_or(EOF)
    }

    /// Peek the next character, and advance the current position if it matches
    #[inline]
    fn next_eq(&mut self, c: char) -> bool {
        let matched = self.peek() == c;
        if matched {
            self.current.chars.next();
        }
        matched
    }

    fn current_offset(&self) -> Span {
        let offset = self.offset();
        Span::new(offset, offset)
    }

    /// Return `IllegalCharacter` Error or `UnexpectedEnd` if EOF
    fn unexpected_err(&mut self) {
        let c = self.peek();
        if c == EOF {
            self.error(diagnostics::UnexpectedEnd(self.current_offset()));
        } else {
            self.error(diagnostics::InvalidCharacter(c, self.current_offset()));
        }
    }

    /// Add string to `SourceAtomSet` and get `TokenValue::Atom`
    fn string_to_token_value(&mut self, s: &'a str) -> TokenValue {
        TokenValue::String(Atom::from(s))
    }

    fn set_numeric_value(&mut self, kind: Kind, src: &'a str) {
        let value = match kind {
            Kind::Decimal | Kind::Binary | Kind::Octal | Kind::Hex => {
                src.strip_suffix('n').map_or_else(
                    || parse_int(src, kind).map(TokenValue::Number),
                    |src| parse_big_int(src, kind).map(TokenValue::BigInt),
                )
            }
            Kind::Float => parse_float(src).map(TokenValue::Number),
            Kind::Undetermined => Ok(TokenValue::Number(std::f64::NAN)),
            _ => unreachable!("{kind}"),
        };

        match value {
            Ok(value) => self.current.token.value = value,
            Err(err) => {
                self.error(diagnostics::InvalidNumber(
                    err,
                    Span::new(self.current.token.start, self.offset()),
                ));
                self.current.token.value = TokenValue::Number(std::f64::NAN);
            }
        };
    }

    /// Read each char and set the current token
    /// Whitespace and line terminators are skipped
    fn read_next_token(&mut self) -> Kind {
        self.current.token.start = self.offset();

        if self.context == LexerContext::JsxChild {
            return self.read_jsx_child();
        }

        loop {
            self.skip_whitespace();

            let offset = self.offset();
            self.current.token.start = offset;
            let builder = AutoCow::new(self);

            if let Some(c) = self.current.chars.next() {
                let kind = self.match_char(c, builder);
                if !kind.is_trivia() {
                    return kind;
                }
            } else {
                return Kind::Eof;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn match_char(&mut self, c: char, mut builder: AutoCow<'a>) -> Kind {
        // fast path for single character tokens
        // '{'  '}'  '('  ')'  '['  ']'  ';' ',' ':' '~'
        let size = c as usize;
        if size < 128 {
            let kind = SINGLE_CHAR_TOKENS[size];
            if kind != Kind::Undetermined {
                return kind;
            }
            // fast path for identifiers
            if is_identifier_start_ascii(c) {
                builder.push_matching(c);
                return self.identifier_name_or_keyword(builder);
            }
        }
        // NOTE: matching order is significant here, by real world occurrences
        // see https://blog.mozilla.org/nnethercote/2011/07/01/faster-javascript-parsing/
        // > the rough order of frequency for different token kinds is as follows:
        // identifiers/keywords, ‘.’, ‘=’, strings, decimal numbers, ‘:’, ‘+’, hex/octal numbers, and then everything else
        match c {
            '.' => {
                let kind = self.read_dot(&mut builder);
                if kind.is_number() {
                    self.set_numeric_value(kind, builder.finish(self));
                }
                kind
            }
            '=' => self.read_equal(),
            '"' | '\'' => {
                if self.context == LexerContext::JsxAttributeValue {
                    self.read_jsx_string_literal(c)
                } else {
                    self.read_string_literal(c)
                }
            }
            '1'..='9' => {
                let kind = self.decimal_literal_after_first_digit(&mut builder);
                self.set_numeric_value(kind, builder.finish(self));
                kind
            }
            '+' => self.read_plus(),
            '-' => self.read_minus().map_or_else(|| self.skip_single_line_comment(), |kind| kind),
            '0' => {
                let kind = self.read_zero(&mut builder);
                self.set_numeric_value(kind, builder.finish(self));
                kind
            }
            '/' => {
                match self.peek() {
                    '/' => {
                        self.current.chars.next();
                        self.skip_single_line_comment()
                    }
                    '*' => {
                        self.current.chars.next();
                        self.skip_multi_line_comment()
                    }
                    _ => {
                        // regex is handled separately, see `next_regex`
                        self.read_slash()
                    }
                }
            }
            '`' => self.read_template_literal(Kind::TemplateHead, Kind::NoSubstitutionTemplate),
            '!' => self.read_exclamation(),
            '%' => self.read_percent(),
            '*' => self.read_star(),
            '&' => self.read_ampersand(),
            '|' => self.read_pipe(),
            '?' => self.read_question(),
            '<' => {
                self.read_left_angle().map_or_else(|| self.skip_single_line_comment(), |kind| kind)
            }
            '^' => self.read_caret(),
            '#' => {
                // https://tc39.es/proposal-hashbang/out.html
                // HashbangComment ::
                //     `#!` SingleLineCommentChars?
                if self.current.token.start == 0 && self.next_eq('!') {
                    self.skip_single_line_comment()
                } else {
                    builder.get_mut_string_without_current_ascii_char(self);
                    self.private_identifier(builder)
                }
            }
            '\\' => {
                builder.force_allocation_without_current_ascii_char(self);
                self.identifier_unicode_escape_sequence(&mut builder, true);
                self.identifier_name_or_keyword(builder)
            }
            c if unicode_id_start::is_id_start_unicode(c) => {
                builder.push_matching(c);
                self.identifier_name_or_keyword(builder)
            }
            c if is_irregular_whitespace(c) => Kind::WhiteSpace,
            c if is_irregular_line_terminator(c) => {
                self.current.token.is_on_new_line = true;
                Kind::NewLine
            }
            _ => {
                self.error(diagnostics::InvalidCharacter(c, self.unterminated_range()));
                Kind::Undetermined
            }
        }
    }

    fn skip_whitespace(&mut self) {
        let c = self.peek();
        let any_newline = c == '\r' || c == '\n';
        let any_white = c == ' ' || c == '\t' || any_newline;
        // Fast path for single non-whitespace
        if any_white {
            self.current.chars.next();
            if any_newline {
                self.current.token.is_on_new_line = true;
            }
        } else {
            return;
        }

        let remaining = self.remaining().as_bytes();
        let mut state = SkipWhitespace::new(self.current.token.is_on_new_line);
        state.simd(remaining);

        // SAFETY: offset is computed to the boundary
        self.current.chars =
            unsafe { std::str::from_utf8_unchecked(&remaining[state.offset..]) }.chars();

        if state.newline {
            self.current.token.is_on_new_line = true;
        }
    }

    /// Section 12.4 Single Line Comment
    #[must_use]
    fn skip_single_line_comment(&mut self) -> Kind {
        while let Some(c) = self.current.chars.next().as_ref() {
            if is_line_terminator(*c) {
                break;
            }
        }
        self.current.token.is_on_new_line = true;
        self.trivia_builder.add_single_line_comment(self.current.token.start, self.offset());
        Kind::Comment
    }

    /// Section 12.4 Multi Line Comment
    #[must_use]
    fn skip_multi_line_comment(&mut self) -> Kind {
        let remaining = self.remaining().as_bytes();
        let newline = self.current.token.is_on_new_line;
        let mut state = SkipMultilineComment::new(newline, remaining);
        state.simd();

        // SAFETY: offset is computed to the boundary
        self.current.chars =
            unsafe { std::str::from_utf8_unchecked(&remaining[state.offset..]) }.chars();

        if state.newline && !newline {
            self.current.token.is_on_new_line = true;
        }

        if !state.found {
            self.error(diagnostics::UnterminatedMultiLineComment(self.unterminated_range()));
            return Kind::Eof;
        }

        self.trivia_builder.add_single_line_comment(self.current.token.start, self.offset());
        Kind::MultiLineComment
    }

    /// Section 12.6.1 Identifier Names
    fn identifier_name(&mut self, mut builder: AutoCow<'a>) -> (bool, &'a str) {
        // ident tail
        loop {
            let c = self.peek();
            if !is_identifier_part(c) {
                if self.next_eq('\\') {
                    builder.force_allocation_without_current_ascii_char(self);
                    self.identifier_unicode_escape_sequence(&mut builder, false);
                    continue;
                }
                break;
            }
            self.current.chars.next();
            builder.push_matching(c);
        }
        let has_escape = builder.has_escape();

        (has_escape, builder.finish(self))
    }

    fn identifier_name_or_keyword(&mut self, builder: AutoCow<'a>) -> Kind {
        let (has_escape, text) = self.identifier_name(builder);
        let (kind, atom) = Kind::match_keyword(text);
        self.current.token.escaped = has_escape;
        self.current.token.value = TokenValue::String(atom);
        kind
    }

    /// Section 12.7 Punctuators
    fn read_dot(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        if self.peek() == '.' && self.peek2() == '.' {
            self.current.chars.next();
            self.current.chars.next();
            return Kind::Dot3;
        }
        if self.peek().is_ascii_digit() {
            builder.push_matching('.');
            self.decimal_literal_after_decimal_point(builder)
        } else {
            Kind::Dot
        }
    }

    /// returns None for `SingleLineHTMLOpenComment` `<!--` in script mode
    fn read_left_angle(&mut self) -> Option<Kind> {
        if self.next_eq('<') {
            if self.next_eq('=') { Some(Kind::ShiftLeftEq) } else { Some(Kind::ShiftLeft) }
        } else if self.next_eq('=') {
            Some(Kind::LtEq)
        } else if self.peek() == '!'
            // SingleLineHTMLOpenComment `<!--` in script mode
            && self.source_type.is_script()
            && self.remaining().starts_with("!--")
        {
            None
        } else {
            Some(Kind::LAngle)
        }
    }

    fn read_right_angle(&mut self) -> Kind {
        if self.next_eq('>') {
            if self.next_eq('>') {
                if self.next_eq('=') { Kind::ShiftRight3Eq } else { Kind::ShiftRight3 }
            } else if self.next_eq('=') {
                Kind::ShiftRightEq
            } else {
                Kind::ShiftRight
            }
        } else if self.next_eq('=') {
            Kind::GtEq
        } else {
            Kind::RAngle
        }
    }

    fn read_equal(&mut self) -> Kind {
        if self.next_eq('=') {
            if self.next_eq('=') { Kind::Eq3 } else { Kind::Eq2 }
        } else if self.next_eq('>') {
            Kind::Arrow
        } else {
            Kind::Eq
        }
    }

    fn read_exclamation(&mut self) -> Kind {
        if self.next_eq('=') {
            if self.next_eq('=') { Kind::Neq2 } else { Kind::Neq }
        } else {
            Kind::Bang
        }
    }

    fn read_plus(&mut self) -> Kind {
        if self.next_eq('+') {
            Kind::Plus2
        } else if self.next_eq('=') {
            Kind::PlusEq
        } else {
            Kind::Plus
        }
    }

    /// returns None for `SingleLineHTMLCloseComment` `-->` in script mode
    fn read_minus(&mut self) -> Option<Kind> {
        if self.next_eq('-') {
            // SingleLineHTMLCloseComment `-->` in script mode
            if self.current.token.is_on_new_line
                && self.source_type.is_script()
                && self.next_eq('>')
            {
                None
            } else {
                Some(Kind::Minus2)
            }
        } else if self.next_eq('=') {
            Some(Kind::MinusEq)
        } else {
            Some(Kind::Minus)
        }
    }

    fn read_caret(&mut self) -> Kind {
        if self.next_eq('=') { Kind::CaretEq } else { Kind::Caret }
    }

    fn read_percent(&mut self) -> Kind {
        if self.next_eq('=') { Kind::PercentEq } else { Kind::Percent }
    }

    fn read_star(&mut self) -> Kind {
        if self.next_eq('*') {
            if self.next_eq('=') { Kind::Star2Eq } else { Kind::Star2 }
        } else if self.next_eq('=') {
            Kind::StarEq
        } else {
            Kind::Star
        }
    }

    fn read_ampersand(&mut self) -> Kind {
        if self.next_eq('&') {
            if self.next_eq('=') { Kind::Amp2Eq } else { Kind::Amp2 }
        } else if self.next_eq('=') {
            Kind::AmpEq
        } else {
            Kind::Amp
        }
    }

    fn read_pipe(&mut self) -> Kind {
        if self.next_eq('|') {
            if self.next_eq('=') { Kind::Pipe2Eq } else { Kind::Pipe2 }
        } else if self.next_eq('=') {
            Kind::PipeEq
        } else {
            Kind::Pipe
        }
    }

    fn read_question(&mut self) -> Kind {
        if self.next_eq('?') {
            if self.next_eq('=') { Kind::Question2Eq } else { Kind::Question2 }
        } else if self.peek() == '.' {
            // parse `?.1` as `?` `.1`
            if self.peek2().is_ascii_digit() {
                Kind::Question
            } else {
                self.current.chars.next();
                Kind::QuestionDot
            }
        } else {
            Kind::Question
        }
    }

    fn read_slash(&mut self) -> Kind {
        if self.next_eq('=') { Kind::SlashEq } else { Kind::Slash }
    }

    fn private_identifier(&mut self, mut builder: AutoCow<'a>) -> Kind {
        let start = self.offset();
        match self.current.chars.next() {
            Some(c) if is_identifier_start_all(c) => {
                builder.push_matching(c);
            }
            Some('\\') => {
                builder.force_allocation_without_current_ascii_char(self);
                self.identifier_unicode_escape_sequence(&mut builder, true);
            }
            Some(c) => {
                self.error(diagnostics::InvalidCharacter(c, Span::new(start, self.offset() - 1)));
                return Kind::Undetermined;
            }
            None => {
                self.error(diagnostics::UnexpectedEnd(Span::new(start, self.offset() - 1)));
                return Kind::Undetermined;
            }
        }
        let (_, name) = self.identifier_name(builder);
        self.current.token.value = self.string_to_token_value(name);
        Kind::PrivateIdentifier
    }

    /// 12.8.3 Numeric Literals with `0` prefix
    fn read_zero(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        match self.peek() {
            'b' | 'B' => self.read_non_decimal(Kind::Binary, builder),
            'o' | 'O' => self.read_non_decimal(Kind::Octal, builder),
            'x' | 'X' => self.read_non_decimal(Kind::Hex, builder),
            c @ ('e' | 'E') => {
                self.current.chars.next();
                builder.push_matching(c);
                self.read_decimal_exponent(builder)
            }
            '.' => {
                self.current.chars.next();
                builder.push_matching('.');
                self.decimal_literal_after_decimal_point_after_digits(builder)
            }
            'n' => {
                self.current.chars.next();
                builder.push_matching('n');
                self.check_after_numeric_literal(Kind::Decimal)
            }
            n if n.is_ascii_digit() => self.read_legacy_octal(builder),
            _ => self.check_after_numeric_literal(Kind::Decimal),
        }
    }

    fn read_non_decimal(&mut self, kind: Kind, builder: &mut AutoCow<'a>) -> Kind {
        let c = self.current.chars.next().unwrap();
        builder.push_matching(c);

        if kind.matches_number_char(self.peek()) {
            let c = self.current.chars.next().unwrap();
            builder.push_matching(c);
        } else {
            self.unexpected_err();
            return Kind::Undetermined;
        }

        loop {
            match self.peek() {
                '_' => {
                    self.current.chars.next();
                    builder.force_allocation_without_current_ascii_char(self);
                    let c = self.peek();
                    if kind.matches_number_char(c) {
                        self.current.chars.next();
                        builder.push_matching(c);
                    } else {
                        self.unexpected_err();
                        return Kind::Undetermined;
                    }
                }
                c if kind.matches_number_char(c) => {
                    self.current.chars.next();
                    builder.push_matching(c);
                }
                _ => break,
            }
        }
        if self.peek() == 'n' {
            self.current.chars.next();
            builder.push_matching('n');
        }
        self.check_after_numeric_literal(kind)
    }

    fn read_legacy_octal(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        let mut kind = Kind::Octal;
        loop {
            match self.peek() {
                '0'..='7' => {
                    self.current.chars.next();
                }
                '8'..='9' => {
                    self.current.chars.next();
                    kind = Kind::Decimal;
                }
                _ => break,
            }
        }

        match self.peek() {
            // allow 08.5 and 09.5
            '.' if kind == Kind::Decimal => {
                self.current.chars.next();
                builder.push_matching('.');
                self.decimal_literal_after_decimal_point_after_digits(builder)
            }
            // allow 08e1 and 09e1
            'e' if kind == Kind::Decimal => {
                self.current.chars.next();
                builder.push_matching('e');
                self.read_decimal_exponent(builder)
            }
            _ => self.check_after_numeric_literal(kind),
        }
    }

    fn decimal_literal_after_first_digit(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        self.read_decimal_digits_after_first_digit(builder);
        if self.next_eq('.') {
            builder.push_matching('.');
            return self.decimal_literal_after_decimal_point_after_digits(builder);
        } else if self.next_eq('n') {
            builder.push_matching('n');
            return self.check_after_numeric_literal(Kind::Decimal);
        }

        let has_exponent = self.optional_exponent(builder);
        self.check_after_numeric_literal(if has_exponent { Kind::Float } else { Kind::Decimal })
    }

    fn read_decimal_exponent(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        let c = self.peek();
        if matches!(c, '+' | '-') {
            self.current.chars.next();
            builder.push_matching(c);
        }
        self.read_decimal_digits(builder);
        Kind::Float
    }

    fn read_decimal_digits(&mut self, builder: &mut AutoCow<'a>) {
        if self.peek().is_ascii_digit() {
            let c = self.current.chars.next().unwrap();
            builder.push_matching(c);
        } else {
            self.unexpected_err();
            return;
        }

        self.read_decimal_digits_after_first_digit(builder);
    }

    fn read_decimal_digits_after_first_digit(&mut self, builder: &mut AutoCow<'a>) {
        loop {
            match self.peek() {
                '_' => {
                    self.current.chars.next();
                    builder.force_allocation_without_current_ascii_char(self);
                    if self.peek().is_ascii_digit() {
                        let c = self.current.chars.next().unwrap();
                        builder.push_matching(c);
                    } else {
                        self.unexpected_err();
                        return;
                    }
                }
                c @ '0'..='9' => {
                    self.current.chars.next();
                    builder.push_matching(c);
                }
                _ => break,
            }
        }
    }

    fn decimal_literal_after_decimal_point(&mut self, builder: &mut AutoCow<'a>) -> Kind {
        self.read_decimal_digits(builder);
        self.optional_exponent(builder);
        self.check_after_numeric_literal(Kind::Float)
    }

    fn decimal_literal_after_decimal_point_after_digits(
        &mut self,
        builder: &mut AutoCow<'a>,
    ) -> Kind {
        self.optional_decimal_digits(builder);
        self.optional_exponent(builder);
        self.check_after_numeric_literal(Kind::Float)
    }

    fn optional_decimal_digits(&mut self, builder: &mut AutoCow<'a>) {
        if self.peek().is_ascii_digit() {
            let c = self.current.chars.next().unwrap();
            builder.push_matching(c);
        } else {
            return;
        }
        self.read_decimal_digits_after_first_digit(builder);
    }

    fn optional_exponent(&mut self, builder: &mut AutoCow<'a>) -> bool {
        if matches!(self.peek(), 'e' | 'E') {
            let c = self.current.chars.next().unwrap();
            builder.push_matching(c);
            self.read_decimal_exponent(builder);
            return true;
        }
        false
    }

    fn check_after_numeric_literal(&mut self, kind: Kind) -> Kind {
        let offset = self.offset();
        // The SourceCharacter immediately following a NumericLiteral must not be an IdentifierStart or DecimalDigit.
        let ch = self.peek();
        if !ch.is_ascii_digit() && !is_identifier_start_all(ch) {
            return kind;
        }
        self.current.chars.next();
        loop {
            let c = self.peek();
            if c != EOF && is_identifier_start_all(c) {
                self.current.chars.next();
            } else {
                break;
            }
        }
        self.error(diagnostics::InvalidNumberEnd(Span::new(offset, self.offset())));
        Kind::Undetermined
    }

    /// 12.8.4 String Literals
    fn read_string_literal(&mut self, delimiter: char) -> Kind {
        let mut builder = AutoCow::new(self);
        loop {
            match self.current.chars.next() {
                None | Some('\r' | '\n') => {
                    self.error(diagnostics::UnterminatedString(self.unterminated_range()));
                    return Kind::Undetermined;
                }
                Some(c @ ('"' | '\'')) => {
                    if c == delimiter {
                        self.current.token.value =
                            self.string_to_token_value(builder.finish_without_push(self));
                        return Kind::Str;
                    }
                    builder.push_matching(c);
                }
                Some('\\') => {
                    let start = self.offset() - 1;
                    let text = builder.get_mut_string_without_current_ascii_char(self);
                    let mut is_valid_escape_sequence = true;
                    self.read_string_escape_sequence(text, false, &mut is_valid_escape_sequence);
                    if !is_valid_escape_sequence {
                        let range = Span::new(start, self.offset());
                        self.error(diagnostics::InvalidEscapeSequence(range));
                    }
                }
                Some(other) => {
                    builder.push_matching(other);
                }
            }
        }
    }

    /// 12.8.5 Regular Expression Literals
    fn read_regex(&mut self, kind: Kind) -> Kind {
        let start = self.current.chars.as_str();
        let mut pattern = String::new_in(self.allocator);
        if kind == Kind::SlashEq {
            pattern.push('=');
        }
        let mut in_escape = false;
        let mut in_character_class = false;
        loop {
            match self.current.chars.next() {
                None => {
                    self.error(diagnostics::UnterminatedRegExp(self.unterminated_range()));
                    return Kind::Undetermined;
                }
                Some(c) if is_line_terminator(c) => {
                    self.error(diagnostics::UnterminatedRegExp(self.unterminated_range()));
                    return Kind::Undetermined;
                }
                Some(c) => {
                    if in_escape {
                        in_escape = false;
                    } else if c == '/' && !in_character_class {
                        break;
                    } else if c == '[' {
                        in_character_class = true;
                    } else if c == '\\' {
                        in_escape = true;
                    } else if c == ']' {
                        in_character_class = false;
                    }
                }
            }
        }

        pattern.push_str(&start[..start.len() - self.current.chars.as_str().len() - 1]);

        let mut flags = RegExpFlags::empty();

        while let ch @ ('$' | '_' | 'a'..='z' | 'A'..='Z' | '0'..='9') = self.peek() {
            self.current.chars.next();
            // dbg!(ch);
            if !ch.is_ascii_lowercase() {
                self.error(diagnostics::RegExpFlag(ch, self.current_offset()));
                continue;
            }
            let flag = match ch {
                'g' => RegExpFlags::G,
                'i' => RegExpFlags::I,
                'm' => RegExpFlags::M,
                's' => RegExpFlags::S,
                'u' => RegExpFlags::U,
                'y' => RegExpFlags::Y,
                'd' => RegExpFlags::D,
                'v' => RegExpFlags::V,
                _ => {
                    self.error(diagnostics::RegExpFlag(ch, self.current_offset()));
                    continue;
                }
            };
            if flags.contains(flag) {
                self.error(diagnostics::RegExpFlagTwice(ch, self.current_offset()));
                continue;
            }
            flags |= flag;
        }

        self.current.token.value =
            TokenValue::RegExp(RegExp { pattern: Atom::from(pattern.as_str()), flags });

        Kind::RegExp
    }

    /// 12.8.6 Template Literal Lexical Components
    fn read_template_literal(&mut self, substitute: Kind, tail: Kind) -> Kind {
        let mut builder = AutoCow::new(self);
        let mut is_valid_escape_sequence = true;
        while let Some(c) = self.current.chars.next() {
            match c {
                '$' if self.peek() == '{' => {
                    if is_valid_escape_sequence {
                        self.current.token.value =
                            self.string_to_token_value(builder.finish_without_push(self));
                    }
                    self.current.chars.next();
                    return substitute;
                }
                '`' => {
                    if is_valid_escape_sequence {
                        self.current.token.value =
                            self.string_to_token_value(builder.finish_without_push(self));
                    }
                    return tail;
                }
                constants::CR => {
                    builder.force_allocation_without_current_ascii_char(self);
                    if self.next_eq(constants::LF) {
                        builder.push_different(constants::LF);
                    }
                }
                '\\' => {
                    let text = builder.get_mut_string_without_current_ascii_char(self);
                    self.read_string_escape_sequence(text, true, &mut is_valid_escape_sequence);
                }
                _ => builder.push_matching(c),
            }
        }
        self.error(diagnostics::UnterminatedString(self.unterminated_range()));
        Kind::Undetermined
    }

    /// `JSXIdentifier` :
    ///   `IdentifierStart`
    ///   `JSXIdentifier` `IdentifierPart`
    ///   `JSXIdentifier` [no `WhiteSpace` or Comment here] -
    fn read_jsx_identifier(&mut self, prev_len: u32) -> Kind {
        let prev_str = &self.source[prev_len as usize..self.offset() as usize];

        let mut builder = AutoCow::new(self);
        loop {
            let c = self.peek();
            if c == '-' || is_identifier_start_all(c) {
                self.current.chars.next();
                builder.push_matching(c);
                loop {
                    let c = self.peek();
                    if is_identifier_part(c) {
                        self.current.chars.next();
                        builder.push_matching(c);
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        let mut s = String::from_str_in(prev_str, self.allocator);
        s.push_str(builder.finish(self));
        self.current.token.value = self.string_to_token_value(s.leak());
        Kind::Ident
    }

    /// [`JSXChild`](https://facebook.github.io/jsx/#prod-JSXChild)
    /// `JSXChild` :
    /// `JSXText`
    /// `JSXElement`
    /// `JSXFragment`
    /// { `JSXChildExpressionopt` }
    fn read_jsx_child(&mut self) -> Kind {
        match self.current.chars.next() {
            Some('<') => Kind::LAngle,
            Some('{') => Kind::LCurly,
            Some(c) => {
                let mut builder = AutoCow::new(self);
                builder.push_matching(c);
                loop {
                    // `>` and `}` are errors in TypeScript but not Babel
                    // let's make this less strict so we can parse more code
                    if matches!(self.peek(), '{' | '<') {
                        break;
                    }
                    if let Some(c) = self.current.chars.next() {
                        builder.push_matching(c);
                    } else {
                        break;
                    }
                }
                self.current.token.value = self.string_to_token_value(builder.finish(self));
                Kind::JSXText
            }
            None => Kind::Eof,
        }
    }

    /// `JSXDoubleStringCharacters` ::
    ///   `JSXDoubleStringCharacter` `JSXDoubleStringCharactersopt`
    /// `JSXDoubleStringCharacter` ::
    ///   `JSXStringCharacter` but not "
    /// `JSXSingleStringCharacters` ::
    ///   `JSXSingleStringCharacter` `JSXSingleStringCharactersopt`
    /// `JSXSingleStringCharacter` ::
    ///   `JSXStringCharacter` but not '
    /// `JSXStringCharacter` ::
    ///   `SourceCharacter` but not one of `HTMLCharacterReference`
    fn read_jsx_string_literal(&mut self, delimiter: char) -> Kind {
        let mut builder = AutoCow::new(self);
        loop {
            match self.current.chars.next() {
                Some(c @ ('"' | '\'')) => {
                    if c == delimiter {
                        self.current.token.value =
                            self.string_to_token_value(builder.finish_without_push(self));
                        return Kind::Str;
                    }
                    builder.push_matching(c);
                }
                Some(other) => {
                    builder.push_matching(other);
                }
                None => {
                    self.error(diagnostics::UnterminatedString(self.unterminated_range()));
                    return Kind::Undetermined;
                }
            }
        }
    }

    /* ---------- utils ---------- */

    /// Identifier `UnicodeEscapeSequence`
    ///   \u `Hex4Digits`
    ///   \u{ `CodePoint` }
    fn identifier_unicode_escape_sequence(
        &mut self,
        builder: &mut AutoCow<'a>,
        check_identifier_start: bool,
    ) {
        let start = self.offset();
        if self.current.chars.next() != Some('u') {
            let range = Span::new(start, self.offset());
            self.error(diagnostics::UnicodeEscapeSequence(range));
            return;
        }

        let value = match self.peek() {
            '{' => self.unicode_code_point(),
            _ => self.surrogate_pair(),
        };

        let Some(value) = value else {
            let range = Span::new(start,self.offset());
            self.error(diagnostics::UnicodeEscapeSequence(range));
            return;
        };

        // For Identifiers, surrogate pair is an invalid grammar, e.g. `var \uD800\uDEA7`.
        let ch = match value {
            SurrogatePair::Astral(..) | SurrogatePair::HighLow(..) => {
                let range = Span::new(start, self.offset());
                self.error(diagnostics::UnicodeEscapeSequence(range));
                return;
            }
            SurrogatePair::CodePoint(code_point) => {
                if let Ok(ch) = char::try_from(code_point) {
                    ch
                } else {
                    let range = Span::new(start, self.offset());
                    self.error(diagnostics::UnicodeEscapeSequence(range));
                    return;
                }
            }
        };

        let is_valid = if check_identifier_start {
            is_identifier_start_all(ch)
        } else {
            is_identifier_part(ch)
        };

        if !is_valid {
            self.error(diagnostics::InvalidCharacter(ch, self.current_offset()));
            return;
        }

        builder.push_different(ch);
    }

    /// String `UnicodeEscapeSequence`
    ///   \u `Hex4Digits`
    ///   \u `Hex4Digits` \u `Hex4Digits`
    ///   \u{ `CodePoint` }
    fn string_unicode_escape_sequence(
        &mut self,
        text: &mut String<'a>,
        is_valid_escape_sequence: &mut bool,
    ) {
        let value = match self.peek() {
            '{' => self.unicode_code_point(),
            _ => self.surrogate_pair(),
        };

        let Some(value) = value else {
            // error raised within the parser by `diagnostics::TemplateLiteral`
            *is_valid_escape_sequence = false;
            return;
        };

        // For strings and templates, surrogate pairs are valid grammar, e.g. `"\uD83D\uDE00" === 😀`
        // values are interpreted as is if they fall out of range
        match value {
            SurrogatePair::CodePoint(code_point) | SurrogatePair::Astral(code_point) => {
                if let Ok(ch) = char::try_from(code_point) {
                    text.push(ch);
                } else {
                    text.push_str("\\u");
                    text.push_str(format!("{code_point:x}").as_str());
                }
            }
            SurrogatePair::HighLow(high, low) => {
                text.push_str("\\u");
                text.push_str(format!("{high:x}").as_str());
                text.push_str("\\u");
                text.push_str(format!("{low:x}").as_str());
            }
        }
    }

    fn unicode_code_point(&mut self) -> Option<SurrogatePair> {
        if !self.next_eq('{') {
            return None;
        }
        let value = self.code_point()?;
        if !self.next_eq('}') {
            return None;
        }
        Some(SurrogatePair::CodePoint(value))
    }

    fn hex_4_digits(&mut self) -> Option<u32> {
        let mut value = 0;
        for _ in 0..4 {
            value = (value << 4) | self.hex_digit()?;
        }
        Some(value)
    }

    fn hex_digit(&mut self) -> Option<u32> {
        let value = match self.peek() {
            c @ '0'..='9' => c as u32 - '0' as u32,
            c @ 'a'..='f' => 10 + (c as u32 - 'a' as u32),
            c @ 'A'..='F' => 10 + (c as u32 - 'A' as u32),
            _ => return None,
        };
        self.current.chars.next();
        Some(value)
    }

    fn code_point(&mut self) -> Option<u32> {
        let mut value = self.hex_digit()?;
        while let Some(next) = self.hex_digit() {
            value = (value << 4) | next;
            if value > 0x0010_FFFF {
                return None;
            }
        }
        Some(value)
    }

    /// Surrogate pairs
    /// See background info:
    ///   * `https://mathiasbynens.be/notes/javascript-encoding#surrogate-formulae`
    ///   * `https://mathiasbynens.be/notes/javascript-identifiers-es6`
    fn surrogate_pair(&mut self) -> Option<SurrogatePair> {
        let high = self.hex_4_digits()?;
        // The first code unit of a surrogate pair is always in the range from 0xD800 to 0xDBFF, and is called a high surrogate or a lead surrogate.
        if !((0xD800..=0xDBFF).contains(&high) && self.peek() == '\\' && self.peek2() == 'u') {
            return Some(SurrogatePair::CodePoint(high));
        }

        self.current.chars.next();
        self.current.chars.next();

        let low = self.hex_4_digits()?;

        // The second code unit of a surrogate pair is always in the range from 0xDC00 to 0xDFFF, and is called a low surrogate or a trail surrogate.
        if !(0xDC00..=0xDFFF).contains(&low) {
            return Some(SurrogatePair::HighLow(high, low));
        }

        // `https://tc39.es/ecma262/#sec-utf16decodesurrogatepair`
        let astral_code_point = (high - 0xD800) * 0x400 + low - 0xDC00 + 0x10000;

        Some(SurrogatePair::Astral(astral_code_point))
    }

    // EscapeSequence ::
    fn read_string_escape_sequence(
        &mut self,
        text: &mut String<'a>,
        in_template: bool,
        is_valid_escape_sequence: &mut bool,
    ) {
        match self.current.chars.next() {
            None => {
                self.error(diagnostics::UnterminatedString(self.unterminated_range()));
            }
            Some(c) => match c {
                // CharacterEscapeSequence
                constants::LF | constants::LS | constants::PS => {}
                constants::CR => {
                    self.next_eq(constants::LF);
                }
                '\'' | '"' | '\\' => text.push(c),
                'b' => text.push('\u{8}'),
                'f' => text.push(constants::FF),
                'n' => text.push(constants::LF),
                'r' => text.push(constants::CR),
                't' => text.push(constants::TAB),
                'v' => text.push(constants::VT),
                // HexEscapeSequence
                'x' => {
                    let value = self
                        .hex_digit()
                        .and_then(|value1| {
                            let value2 = self.hex_digit()?;
                            Some((value1, value2))
                        })
                        .map(|(value1, value2)| (value1 << 4) | value2)
                        .and_then(|value| char::try_from(value).ok());
                    value.map_or_else(
                        || {
                            *is_valid_escape_sequence = false;
                        },
                        |c| {
                            text.push(c);
                        },
                    );
                }
                // UnicodeEscapeSequence
                'u' => {
                    self.string_unicode_escape_sequence(text, is_valid_escape_sequence);
                }
                // 0 [lookahead ∉ DecimalDigit]
                '0' if !self.peek().is_ascii_digit() => text.push('\0'),
                // Section 12.8.4 String Literals
                // LegacyOctalEscapeSequence
                // NonOctalDecimalEscapeSequence
                a @ '0'..='7' if !in_template => {
                    let mut num = String::new_in(self.allocator);
                    num.push(a);
                    match a {
                        '4'..='7' => {
                            if matches!(self.peek(), '0'..='7') {
                                let b = self.current.chars.next().unwrap();
                                num.push(b);
                            }
                        }
                        '0'..='3' => {
                            if matches!(self.peek(), '0'..='7') {
                                let b = self.current.chars.next().unwrap();
                                num.push(b);
                                if matches!(self.peek(), '0'..='7') {
                                    let c = self.current.chars.next().unwrap();
                                    num.push(c);
                                }
                            }
                        }
                        _ => {}
                    }

                    let value =
                        char::from_u32(u32::from_str_radix(num.as_str(), 8).unwrap()).unwrap();
                    text.push(value);
                }
                '0' if in_template && self.peek().is_ascii_digit() => {
                    self.current.chars.next();
                    // error raised within the parser by `diagnostics::TemplateLiteral`
                    *is_valid_escape_sequence = false;
                }
                // NotEscapeSequence :: DecimalDigit but not 0
                '1'..='9' if in_template => {
                    // error raised within the parser by `diagnostics::TemplateLiteral`
                    *is_valid_escape_sequence = false;
                }
                other => {
                    // NonOctalDecimalEscapeSequence \8 \9 in strict mode
                    text.push(other);
                }
            },
        }
    }
}

#[derive(Debug)]
enum SurrogatePair {
    // valid \u Hex4Digits \u Hex4Digits
    Astral(u32),
    // valid \u Hex4Digits
    CodePoint(u32),
    // invalid \u Hex4Digits \u Hex4Digits
    HighLow(u32, u32),
}
