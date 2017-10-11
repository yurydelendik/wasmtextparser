use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::str;

use lexer::{WatLexer, WatTokenType};
use wat::{WatParser, WatParserState};

mod lexer;
mod wat;

fn main() {
    let wat = &_read_wat().unwrap();
    let mut parser = WatParser::new(wat);
    loop {
        let s = parser.parse();
        println!("{:?}", s);
        if let WatParserState::End = *s {
            break;
        }
        if let WatParserState::Error(err) = *s {
            panic!("parse failed: {}", err.message);
        }
    }
}

fn _read_wat() -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut f = File::open("t.wat")?;
    f.read_to_end(&mut data)?;
    Ok(data)
}
