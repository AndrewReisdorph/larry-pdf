// #![feature(collections)]

use std::io::{prelude::*, Cursor};
use std::fs::File;
use env_logger::{Builder, Target};

pub mod tokenizer;
pub mod reader;
pub mod pdf;
pub mod page;
pub mod content_stream_lexer;
pub mod text;

fn main() {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(log::LevelFilter::Debug)
        .init();

    let mut f = File::open("/Users/andrew/Downloads/63dcb628-666e-457e-a989-3e9ca38f6b78.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/Borrower 210001967312 - 1098-E Tax Form (1).pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/779503749_2022-05-11.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/bill-8743148.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/centurylink.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/documents.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/Loan 360001863193 - 10_28_2010 Line Of Credit Statement.pdf").unwrap();
    // let mut f = File::open("/Users/andrew/Downloads/ug527-brd4188b-user-guide.pdf").unwrap();
    let mut bytes: Vec<u8> = Vec::new();
    f.read_to_end(&mut bytes);
    let mut cursor = Cursor::new(bytes);
    let file_size = f.stream_position().unwrap();
    println!("file size: {}", file_size);
    println!("cursor pos: {}", cursor.stream_position().unwrap());

    let mut tokenizer: tokenizer::Tokenizer<Cursor<Vec<u8>>> = tokenizer::Tokenizer::new(cursor);
    let mut pdf_reader = reader::Reader::new(tokenizer);

    pdf_reader.read();

    println!("DONE");
}
