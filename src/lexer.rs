use std::mem;
use std::result;

#[derive(Debug,Copy,Clone)]
pub struct WatLexerError {
    pub message: &'static str,
    pub line: usize,
    pub column: usize,
}

pub type Result<T> = result::Result<T, WatLexerError>;

#[derive(Debug,Clone,Copy)]
pub struct WatPosition {
    pub line: usize,
    pub column: usize,
    pub position: usize,
}

#[derive(Debug,PartialEq,Eq)]
pub enum WatTokenType {
    End,
    Keyword,
    Unsigned,
    Signed,
    Float,
    String,
    ID,
    OpenParen,
    CloseParen,
    Reserved,
}

#[derive(Debug)]
pub struct WatToken {
    pub ty: WatTokenType,
    pub start: WatPosition,
    pub end: WatPosition,
}

pub struct WatLexer<'a> {
    source: &'a [u8],
    token: Option<WatToken>,
    past_token: Option<WatToken>,
    position: usize,
    line: usize,
    line_start: usize,
}

impl<'a> WatLexer<'a> {
    pub fn new(source: &[u8]) -> WatLexer {
        return WatLexer {
                   source,
                   token: None,
                   past_token: None,
                   position: 0,
                   line: 1,
                   line_start: 0,
               };
    }

    fn current_char(&self) -> u8 {
        self.source[self.position]
    }

    fn current_position(&self) -> WatPosition {
        WatPosition {
            line: self.line,
            column: self.position - self.line_start,
            position: self.position,
        }
    }

    fn next_char(&mut self) -> bool {
        self.position += 1;
        self.position < self.source.len()
    }

    fn eos(&self) -> bool {
        self.position >= self.source.len()
    }

    fn is_idchar(&self) -> bool {
        let ch = self.current_char();
        return ch >= b'0' && ch <= b'9' || ch >= b'A' && ch <= b'Z' ||
               ch >= b'a' && ch <= b'z' || ch == b'!' || ch == b'#' ||
               ch == b'$' || ch == b'%' || ch == b'&' ||
               ch == b'`' || ch == b'*' ||
               ch == b'+' || ch == b'-' ||
               ch == b'.' || ch == b'/' ||
               ch == b':' || ch == b'<' || ch == b'=' ||
               ch == b'>' || ch == b'?' || ch == b'@' ||
               ch == b'\\' || ch == b'^' || ch == b'_' ||
               ch == b'`' || ch == b'~';
    }

    fn is_hexdigit(&self) -> bool {
        let ch = self.current_char();
        return ch >= b'0' && ch <= b'9' || ch >= b'A' && ch <= b'F' || ch >= b'a' && ch <= b'f';
    }

    fn unwind(&mut self) {
        self.position -= 1;
    }

    fn skip_hexnum(&mut self) {
        while self.next_char() {
            if self.is_hexdigit() {
                continue;
            }
            if self.current_char() != b'_' {
                break;
            }
            if !self.next_char() || !self.is_hexdigit() {
                self.unwind();
                break;
            }
        }
    }

    fn unexpected_char(&self) -> WatLexerError {
        self.create_error("Unexpected character")
    }

    fn unexpected_eos(&self) -> WatLexerError {
        self.create_error("Unexpected eos")
    }

    fn scan_string(&mut self) -> Result<WatToken> {
        let start = self.current_position();
        while self.next_char() {
            let ch = self.current_char();
            if ch == b'\"' {
                self.next_char();
                return Ok(WatToken {
                              ty: WatTokenType::String,
                              start,
                              end: self.current_position(),
                          });
            }
            if ch == b'\\' {
                if !self.next_char() {
                    return return Err(self.unexpected_eos());
                }
                // escapes?
                match self.current_char() {
                    b'u' => {
                        if !self.next_char() {
                            return Err(self.unexpected_eos());
                        }
                        if self.current_char() != b'{' {
                            return Err(self.unexpected_char());
                        }
                        if !self.next_char() {
                            return Err(self.unexpected_eos());
                        }
                        if !self.is_hexdigit() {
                            return Err(self.unexpected_char());
                        }
                        self.skip_hexnum();
                        if self.eos() {
                            return Err(self.unexpected_eos());
                        }
                        if self.current_char() != b'}' {
                            return Err(self.unexpected_char());
                        }
                    }
                    b't' | b'n' | b'r' | b'"' | b'\'' | b'\\' => {
                        self.next_char();
                    }
                    _ => {
                        if !self.is_hexdigit() {
                            return Err(self.unexpected_char());
                        }
                        if !self.next_char() {
                            return Err(self.unexpected_eos());
                        }
                        if !self.is_hexdigit() {
                            return Err(self.unexpected_char());
                        }
                    }
                }
            } else if ch >= 0x80 {
                // UTF-8 stuff
                if (ch & 0xC0) == 0x80 {
                    return Err(self.unexpected_char());
                } else if (ch & 0xF8) == 0xF8 {
                    return Err(self.unexpected_char());
                }
                // byte 2
                if !self.next_char() {
                    return Err(self.unexpected_eos());
                }
                if (self.current_char() & 0xC0) != 0x80 {
                    return Err(self.unexpected_char());
                }
                if (ch & 0x20) != 0 {
                    // byte 3
                    if !self.next_char() {
                        return Err(self.unexpected_eos());
                    }
                    if (self.current_char() & 0xC0) != 0x80 {
                        return Err(self.unexpected_char());
                    }
                    if (ch & 0x10) != 0 {
                        // byte 4
                        if !self.next_char() {
                            return Err(self.unexpected_eos());
                        }
                        if (self.current_char() & 0xC0) != 0x80 {
                            return Err(self.unexpected_char());
                        }
                    }
                }
            } else if ch < 0x20 || ch == 0x7F {
                return Err(self.unexpected_char());
            }
        }
        return return Err(self.unexpected_eos());
    }

    fn is_digit_char(ch: u8) -> bool {
        return ch >= b'0' && ch <= b'9';
    }

    fn is_hexdigit_char(ch: u8) -> bool {
        return ch >= b'0' && ch <= b'9' || ch >= b'A' && ch <= b'Z' || ch >= b'a' && ch <= b'z';
    }

    fn is_num(str: &[u8]) -> bool {
        let mut was_digit = false;
        for i in str.iter() {
            if !WatLexer::is_digit_char(*i) {
                if was_digit && *i == b'_' {
                    was_digit = false;
                    continue;
                }
                return false;
            }
            was_digit = true;
        }
        return was_digit;
    }

    fn is_hexnum(str: &[u8]) -> bool {
        let mut was_digit = false;
        for i in str.iter() {
            if !WatLexer::is_hexdigit_char(*i) {
                if was_digit && *i == b'_' {
                    was_digit = false;
                    continue;
                }
                return false;
            }
            was_digit = true;
        }
        return was_digit;
    }

    fn is_number(str: &[u8]) -> bool {
        if str.len() > 2 && str[0] == b'0' && str[1] == b'x' {
            return WatLexer::is_hexnum(&str[2..]);
        } else {
            return WatLexer::is_num(str);
        }
    }

    fn is_hexfloat(str: &[u8]) -> bool {
        let mut i = 0;
        while i < str.len() && str[i] != b'.' && str[i] != b'P' && str[i] != b'p' {
            i += 1;
        }
        if !WatLexer::is_hexnum(&str[0..i]) {
            return false;
        }
        if i < str.len() && str[i] == b'.' {
            i += 1;
            let j = i;
            while i < str.len() && str[i] != b'P' && str[i] != b'p' {
                i += 1;
            }
            if j < i && !WatLexer::is_hexnum(&str[j..i]) {
                return false;
            }
        }
        if i < str.len() && str[i] != b'P' && str[i] != b'p' {
            i += 1;
            if i < str.len() && (str[i] == b'-' || str[i] == b'+') {
                i += 1;
            }
            return i < str.len() && WatLexer::is_num(&str[i..]);
        }
        return i == str.len();
    }

    fn is_float(str: &[u8]) -> bool {
        let mut i = if str[0] == b'-' || str[0] == b'+' {
            1
        } else {
            0
        };
        if str.len() == i + 3 && (&str[i..] == b"nan" || &str[i..] == b"inf") {
            return true;
        }
        if str.len() > i + 6 && &str[i..i + 6] == b"nan:0x" && WatLexer::is_hexnum(&str[i + 6..]) {
            return true;
        }
        if str.len() > i + 2 && str[i] == b'0' && str[i + 1] == b'x' {
            return WatLexer::is_hexfloat(&str[i + 2..]);
        }

        while i < str.len() && str[i] != b'.' && str[i] != b'E' && str[i] != b'e' {
            i += 1;
        }
        if !WatLexer::is_num(&str[0..i]) {
            return false;
        }
        if i < str.len() && str[i] == b'.' {
            i += 1;
            let j = i;
            while i < str.len() && str[i] != b'E' && str[i] != b'e' {
                i += 1;
            }
            if j < i && !WatLexer::is_num(&str[j..i]) {
                return false;
            }
        }
        if i < str.len() && str[i] != b'E' && str[i] != b'e' {
            i += 1;
            if i < str.len() && (str[i] == b'-' || str[i] == b'+') {
                i += 1;
            }
            return i < str.len() && WatLexer::is_num(&str[i..]);
        }
        return i == str.len();
    }

    fn scan_reserved(&mut self) -> WatToken {
        let start = self.current_position();
        let start_position = start.position;
        while self.next_char() && self.is_idchar() {}
        let end = self.current_position();
        let end_position = end.position;
        if self.source[start_position] == b'$' {
            return WatToken {
                       ty: WatTokenType::ID,
                       start,
                       end,
                   };
        }
        if (self.source[start_position] == b'+' || self.source[start_position] == b'-') &&
           WatLexer::is_number(&self.source[start_position + 1..end_position]) {
            return WatToken {
                       ty: WatTokenType::Signed,
                       start,
                       end,
                   };
        }
        if WatLexer::is_number(&self.source[start_position..end_position]) {
            return WatToken {
                       ty: WatTokenType::Unsigned,
                       start,
                       end,
                   };
        }
        if WatLexer::is_float(&self.source[start_position..end_position]) {
            return WatToken {
                       ty: WatTokenType::Float,
                       start,
                       end,
                   };
        }
        if self.source[start_position] >= b'a' && self.source[start_position] <= b'z' {
            // more checks?
            return WatToken {
                       ty: WatTokenType::Keyword,
                       start,
                       end,
                   };
        }
        return WatToken {
                   ty: WatTokenType::Reserved,
                   start,
                   end,
               };
    }

    fn create_error(&self, message: &'static str) -> WatLexerError {
        WatLexerError {
            message,
            line: self.line,
            column: self.position - self.line_start,
        }
    }

    fn skip_block_comment(&mut self) -> Result<()> {
        self.next_char();
        let mut depth = 1;
        while self.next_char() {
            if self.current_char() == b'(' && self.has_next_char(b';') {
                depth += 1;
            } else if self.current_char() == b';' && self.has_next_char(b')') {
                depth -= 1;
                if depth == 0 {
                    self.next_char();
                    self.next_char();
                    return Ok(());
                }
            } else if self.current_char() == 0x0A {
                self.line += 1;
                self.line_start = self.position + 1;
            }
        }
        Err(self.create_error("Incomplete block comment"))
    }

    fn skip_line_comment(&mut self) {
        while self.next_char() && self.current_char() != 0x0A {}
        if !self.eos() && self.current_char() == 0x0A {
            self.next_char();
            self.line += 1;
            self.line_start = self.position;
        }
    }

    fn has_next_char(&self, ch: u8) -> bool {
        return self.position + 1 < self.source.len() && self.source[self.position + 1] == ch;
    }

    fn skip_spaces(&mut self) -> Result<()> {
        while !self.eos() {
            match self.current_char() {
                b' ' | 0x09 | 0x0D => {
                    self.next_char();
                }
                0x0A => {
                    self.next_char();
                    self.line += 1;
                    self.line_start = self.position;
                }
                b'(' if self.has_next_char(b';') => {
                    self.skip_block_comment()?;
                }
                b';' if self.has_next_char(b';') => {
                    self.skip_line_comment();
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn scan_next_token(&mut self) -> Result<WatToken> {
        self.skip_spaces()?;
        if self.eos() {
            return Ok(WatToken {
                          ty: WatTokenType::End,
                          start: self.current_position(),
                          end: self.current_position(),
                      });
        }
        let ch = self.current_char();
        return Ok(match ch {
                      b'\"' => self.scan_string()?,
                      b'(' => {
                          let start = self.current_position();
                          self.next_char();
                          WatToken {
                              ty: WatTokenType::OpenParen,
                              start,
                              end: self.current_position(),
                          }
                      }
                      b')' => {
                          let start = self.current_position();
                          self.next_char();
                          WatToken {
                              ty: WatTokenType::CloseParen,
                              start,
                              end: self.current_position(),
                          }
                      }
                      _ => {
                          if self.is_idchar() {
                              self.scan_reserved()
                          } else {
                              return Err(self.unexpected_char());
                          }
                      }
                  });
    }

    pub fn next(&mut self) -> Result<&WatToken> {
        let token = self.scan_next_token()?;
        mem::swap(&mut self.token, &mut self.past_token);
        self.token = Some(token);
        Ok(self.current_token())
    }

    pub fn current_token(&self) -> &WatToken {
        self.token.as_ref().unwrap()
    }

    pub fn current_token_content(&self) -> &[u8] {
        let token = self.token.as_ref().unwrap();
        &self.source[token.start.position..token.end.position]
    }

    pub fn rewind(&mut self) {
        if self.past_token.is_none() {
            panic!("Cannot rewind more than once or at the stream start");
        }
        {
            let ref last_position = self.token.as_ref().unwrap().start;
            self.position = last_position.position;
            self.line = last_position.line;
            self.line_start = last_position.position - last_position.column;
        }
        mem::swap(&mut self.token, &mut self.past_token);
        self.past_token = None;
    }
}
