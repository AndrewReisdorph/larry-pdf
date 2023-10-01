// use core::slice::SlicePattern;
use std::{io::{Cursor, Write}, borrow::Borrow, fs::File};

use crate::{pdf::{PDFObject, PDFValue}, tokenizer::{Tokenizer, self, PDFTokenize}, content_stream_lexer::{parse, ContentToken}, text::{get_text_objects, compile_grouped_text}};

use log::debug;


#[derive(Debug, Clone)]
pub struct PDFPage {
    pub object: PDFObject,
    pub contents: PDFObject
}


impl PDFPage {
    pub fn get_text(&self, temp: i32) {
        // println!("{:?}", self);
        let stream_bytes = self.contents.value.stream().unwrap().decompress();

        // let filename = format!("page_{}.bin",temp);
        // let mut file = File::create(filename).unwrap();
        // file.write_all(&stream_bytes);
        
        // println!("{}\n\n", String::from_utf8_lossy(&stream_bytes));
        let tokens = parse(stream_bytes.as_slice());
        let positioned_text = get_text_objects(&tokens);
        let grouped_text = compile_grouped_text(positioned_text.as_slice());
        // println!("==============\nThe Tokens\n==============\n");
        // for token in tokens {
        //     match token {
        //         ContentToken::ShowTextString(text) => {
        //             println!("TEXT: {}", text);
        //         },
        //         t => println!("{:?}", t)
        //     }
        // }

        // Get any text contained in the pages X-Objects
        // println!("Object: {:?}",self.object.value);
        // match &self.object.value {
        //     PDFValue::Dictionary(dict) => {
        //         println!("Found dict: {:?}", dict);
        //         let contents = dict.get("Contents").unwrap();

        //     },
        //     _ => {}
        // }

        panic!();
    }
}
