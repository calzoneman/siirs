use anyhow::{anyhow, bail, Result};
use std::str::FromStr;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    iter::Peekable,
};

use crate::sii::{
    parser::DataBlock,
    value::{Value, ID},
};

// Workaround for Option<Result> awkwardness -- map None to EOFError for
// the inner iterator, then let the outer iterator unwrap it.  Easier than
// trying to switch over to nightly and create a custom try_trait_v2.
#[derive(Debug)]
struct EOFError {}

impl Display for EOFError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl std::error::Error for EOFError {}

macro_rules! eof {
    () => {{
        return Err(EOFError {}.into());
    }};
}

macro_rules! peek {
    ($e:expr) => {
        match $e.peek() {
            Some(Ok(v)) => v,
            Some(Err(e)) => return Err(anyhow!("read error: {e}")),
            None => eof!(),
        }
    };
}

macro_rules! next {
    ($e:expr) => {
        match $e.next() {
            Some(Ok(v)) => v,
            Some(Err(e)) => return Err(anyhow!("read error: {e}")),
            None => eof!(),
        }
    };
}

macro_rules! take_string {
    ($iter:expr, $p:pat) => {{
        let mut tmp = Vec::new();
        loop {
            match *peek!($iter) {
                c @ ($p) => {
                    next!($iter);
                    tmp.push(c);
                }
                _ => break,
            }
        }
        String::from_utf8(tmp)?
    }};
}

macro_rules! expect_char {
    ($next:expr, $p:literal) => {
        match $next {
            $p => {}
            x => bail!("expected '{}' but found '{}'", x as char, $p),
        }
    };
}

#[derive(Debug)]
enum Token {
    Identifier(String),
    QuotedString(String),
    // All numerics without decimals parsed as u64 for simplicity; this is good
    // enough for the achievements.sii file (which does not model any signed
    // numbers).
    Integer(u64),
    Float(f32),
    Boolean(bool),
    LeftBrace,
    RightBrace,
    Colon,
    LeftRightBracket,
}

struct Lexer<I>(Peekable<I>)
where
    I: Iterator<Item = std::io::Result<u8>>;

impl<I: Iterator<Item = std::io::Result<u8>>> Lexer<I> {
    fn next_inner(&mut self) -> Result<Token> {
        self.skip_whitespace()?;

        match peek!(self.0) {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' => {
                self.read_identifier_or_number()
            }
            b'{' => {
                next!(self.0);
                Ok(Token::LeftBrace)
            }
            b'}' => {
                next!(self.0);
                Ok(Token::RightBrace)
            }
            b':' => {
                next!(self.0);
                Ok(Token::Colon)
            }
            b'[' => self.read_left_right_bracket(),
            b'"' => self.read_quoted_string(),
            x => Err(anyhow!("unexpected '{}'", *x as char)),
        }
    }

    fn read_identifier_or_number(&mut self) -> Result<Token> {
        // The text sii format annoyingly permits bare strings beginning with
        // digits, for example achievement_name: 5_jobs_in_a_row.
        // Take the whole thing as a string, and then if it only contains digits
        // and dots try to parse it as a number.
        let chars = take_string!(self.0, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_');

        if chars.contains(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '_')) {
            match chars.as_str() {
                "true" => Ok(Token::Boolean(true)),
                "false" => Ok(Token::Boolean(false)),
                _ => Ok(Token::Identifier(chars)),
            }
        } else {
            if chars.contains(".") {
                f32::from_str(chars.as_str())
                    .map(|f| Token::Float(f))
                    .map_err(|e| e.into())
            } else {
                u64::from_str(chars.as_str())
                    .map(|n| Token::Integer(n))
                    .map_err(|e| e.into())
            }
        }
    }

    fn read_left_right_bracket(&mut self) -> Result<Token> {
        expect_char!(next!(self.0), b'[');
        expect_char!(next!(self.0), b']');
        Ok(Token::LeftRightBracket)
    }

    fn read_quoted_string(&mut self) -> Result<Token> {
        expect_char!(next!(self.0), b'"');
        // all bytes except "
        let s = take_string!(self.0, 0u8..=33u8 | 35u8..=255u8);
        expect_char!(next!(self.0), b'"');
        Ok(Token::QuotedString(s))
    }

    fn skip_whitespace(&mut self) -> Result<()> {
        loop {
            match peek!(self.0) {
                b'#' => self.skip_comment()?,
                b' ' | b'\t' | b'\r' | b'\n' => {
                    next!(self.0);
                }
                _ => break,
            }
        }

        Ok(())
    }

    // Technically I think the format supports // single-line comments and
    // /* */ multiline comments, but achievements.sii doesn't use them.
    fn skip_comment(&mut self) -> Result<()> {
        let mut last = false;
        while !last {
            if *peek!(self.0) == b'\n' {
                last = true;
            }
            next!(self.0);
        }

        Ok(())
    }
}

impl<I: Iterator<Item = std::io::Result<u8>>> Iterator for Lexer<I> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_inner() {
            Ok(t) => {
                // dbg!(&t);
                Some(Ok(t))
            }
            Err(e) => match e.downcast_ref::<EOFError>() {
                Some(_) => None,
                None => Some(Err(e)),
            },
        }
    }
}

macro_rules! expect_token {
    ($e:expr, $p:pat $(if $guard:expr)?) => {
        match $e {
            $p $(if $guard)? => { },
            t => bail!("expected {} but found {:?}", stringify!($p), t)
        }
    };
}

macro_rules! match_token {
    ($e:expr, $tt:ident) => {
        match $e {
            Token::$tt(v) => v,
            t => bail!("unexpected {:?}", t),
        }
    };
}

/// A good-enough textual sii file parser -- good enough to can parse achievements.sii
// TODO: make pub, make the new() handle creating a lexer from file
struct Parser<L: Iterator<Item = Result<Token>>> {
    lexer: Peekable<L>,
}

impl<L: Iterator<Item = Result<Token>>> Parser<L> {
    pub fn new(mut lexer: L) -> Result<Self> {
        expect_token!(next!(lexer), Token::Identifier(ref s) if s == "SiiNunit");
        expect_token!(next!(lexer), Token::LeftBrace);

        Ok(Self {
            lexer: lexer.peekable(),
        })
    }

    fn next_inner(&mut self) -> Result<DataBlock> {
        match self.lexer.peek() {
            Some(Ok(Token::RightBrace)) => eof!(),
            Some(Ok(Token::Identifier(_))) => self.read_struct(),
            Some(Ok(t)) => Err(anyhow!("unexpected {:?}", t)),
            Some(Err(e)) => Err(anyhow!("error in tokenizer: {e}")),
            None => Err(anyhow!("unexpected end of token stream")),
        }
    }

    // struct_name : struct_id { fields }
    fn read_struct(&mut self) -> Result<DataBlock> {
        let struct_name = match_token!(next!(self.lexer), Identifier);
        expect_token!(next!(self.lexer), Token::Colon);
        let struct_id = ID::try_from(match_token!(next!(self.lexer), Identifier))?;
        expect_token!(next!(self.lexer), Token::LeftBrace);
        // The textual sii format builds up arrays one element at a time.
        // Track arrays of individual Values as we parse them and convert to
        // a Value::<something>Array type at the end of the struct definition.
        let mut arrays: HashMap<String, Vec<Value>> = HashMap::new();
        let mut fields: HashMap<String, Value> = HashMap::new();

        loop {
            match peek!(self.lexer) {
                Token::RightBrace => {
                    next!(self.lexer);
                    break;
                }
                _ => {
                    let field_name = match_token!(next!(self.lexer), Identifier);
                    // field[]: value
                    let is_array = match peek!(self.lexer) {
                        Token::LeftRightBracket => {
                            next!(self.lexer);
                            true
                        }
                        _ => false,
                    };
                    expect_token!(next!(self.lexer), Token::Colon);
                    let field_value = match next!(self.lexer) {
                        // In the binary format, there are strings, encoded
                        // strings, and IDs.  In the text format, there is
                        // ambiguity because we lack a predefined schema.
                        // Return them all as String for simplicity; the caller
                        // can use ID::try_from() as needed if the field is
                        // semantically an ID.
                        Token::Identifier(i) => Value::String(i),
                        Token::QuotedString(s) => Value::String(s),
                        Token::Integer(i) => Value::UInt64(i),
                        Token::Float(f) => Value::Single(f),
                        Token::Boolean(b) => Value::ByteBool(b),
                        t => bail!("unexpected {:?}", t),
                    };

                    if is_array {
                        if !arrays.contains_key(&field_name) {
                            arrays.insert(field_name.clone(), Vec::new());
                        }

                        arrays
                            .get_mut(&field_name)
                            .expect("inserted if it didn't exist")
                            .push(field_value);
                    } else {
                        fields.insert(field_name, field_value);
                    }
                }
            }
        }

        for (name, values) in arrays {
            let array_value = Value::try_from(values)?;
            fields.insert(name, array_value);
        }

        Ok(DataBlock {
            id: struct_id,
            struct_name: struct_name,
            fields: fields,
        })
    }
}

impl<L: Iterator<Item = Result<Token>>> Iterator for Parser<L> {
    type Item = Result<DataBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: see if this ugliness can be hidden in a macro
        match self.next_inner() {
            Ok(t) => Some(Ok(t)),
            Err(e) => match e.downcast_ref::<EOFError>() {
                Some(_) => None,
                None => Some(Err(e)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read};

    use super::{Lexer, Parser};

    // TODO: embed a subset of this file to test the parser instead of relying
    // on local files to be there
    #[test]
    fn test_parse_file() {
        let f = File::open("5C075DC23D8D177-achievements.sii").unwrap();
        let lex = Lexer(f.bytes().peekable());
        let mut parser = Parser::new(lex).unwrap();
        loop {
            match parser.next() {
                Some(Ok(t)) => {
                    dbg!(t);
                }
                Some(Err(e)) => {
                    panic!("{}", e);
                }
                None => break,
            }
        }
    }
}
