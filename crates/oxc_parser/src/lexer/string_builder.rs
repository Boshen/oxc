// Copied from https://github.com/mozilla-spidermonkey/jsparagus/blob/master/crates/parser/src/lexer.rs#L2256

use oxc_allocator::String;

use crate::lexer::Lexer;

#[derive(Debug)]
pub struct AutoCow<'a> {
    pub start: &'a str,
    pub value: Option<String<'a>>,
}

impl<'a> AutoCow<'a> {
    pub fn new(lexer: &Lexer<'a>) -> Self {
        let start = lexer.remaining();
        AutoCow { start, value: None }
    }

    // Push a char that matches lexer.chars().next()
    pub fn push_matching(&mut self, c: char) {
        if let Some(text) = &mut self.value {
            text.push(c);
        }
    }

    // Push a different character than lexer.chars().next().
    // force_allocation_without_current_ascii_char must be called before this.
    pub fn push_different(&mut self, c: char) {
        debug_assert!(self.value.is_some());
        self.value.as_mut().unwrap().push(c);
    }

    // Force allocation of a String, excluding the current ASCII character,
    // and return the reference to it
    pub fn get_mut_string_without_current_ascii_char<'b>(
        &'b mut self,
        lexer: &'_ Lexer<'a>,
    ) -> &'b mut String<'a> {
        self.force_allocation_without_current_ascii_char(lexer);
        self.value.as_mut().unwrap()
    }

    // Force allocation of a String, excluding the current ASCII character.
    pub fn force_allocation_without_current_ascii_char(&mut self, lexer: &'_ Lexer<'a>) {
        if self.value.is_some() {
            return;
        }
        self.value = Some(String::from_str_in(
            &self.start[..self.start.len() - lexer.remaining().len() - 1],
            lexer.allocator,
        ));
    }

    // Check if the string contains a different character, such as an escape sequence
    pub fn has_escape(&self) -> bool {
        self.value.is_some()
    }

    pub fn finish(mut self, lexer: &Lexer<'a>) -> &'a str {
        match self.value.take() {
            Some(s) => s.leak(),
            None => &self.start[..self.start.len() - lexer.remaining().len()],
        }
    }

    // Just like finish, but without pushing current char.
    pub fn finish_without_push(mut self, lexer: &Lexer<'a>) -> &'a str {
        match self.value.take() {
            Some(s) => s.leak(),
            None => &self.start[..self.start.len() - lexer.remaining().len() - 1],
        }
    }
}
