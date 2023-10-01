use std::borrow::BorrowMut;

use crate::{content_stream_lexer::ContentToken, text};

#[derive(Debug, Clone)]
pub struct PositionedText {
    pub text: String,
    pub x: f64,
    pub y: f64
}

#[derive(Debug, Clone)]
pub struct TextObjectContent {
    pub positioned_text: Vec<PositionedText>
}

pub fn get_text_objects(tokens:  &Vec<ContentToken>) -> Vec<TextObjectContent> {
    let mut token_iter = tokens.iter();

    let mut in_text_object = false;
    let mut text_matrix: Option<Vec<f64>> = None;

    let mut text_objects: Vec<TextObjectContent> = vec![];
    let mut current_text_object = TextObjectContent {
        positioned_text: vec![]
    };

    loop {
        let token = token_iter.next();
        if token.is_none() {
            break;
        }
        let token = token.unwrap();

        if in_text_object {
            match token {
                ContentToken::BeginTextObject => {
                    panic!("Unhandled nested text object");
                },
                ContentToken::EndTextObject => {
                    in_text_object = false;
                    //TODO: This clone is bad :(
                    text_objects.push(current_text_object.clone());
                },
                ContentToken::SetTextMatrix(matrix) => {
                    text_matrix = Some(matrix.clone());
                },
                ContentToken::TextFont(_) => {},
                ContentToken::ShowTextString(text) => {
                    let mut x: f64 = 0.0;
                    let mut y: f64 = 0.0;

                    if text_matrix.is_none() {
                        panic!("No text matrix set");
                    }

                    let matrix = text_matrix.clone().unwrap();
                    if matrix.len() == 6 {
                        x = matrix[4];
                        y = matrix[5];
                    } else {
                        panic!("Unexpected text matrix length: {}", matrix.len());
                    }

                    current_text_object.positioned_text.push(PositionedText {
                        text: text.clone(), x, y
                    })
                },
                unhandled_token => {
                    panic!("Unhandled token in text object {:?}", unhandled_token);
                },
            }
        } else {
            match token {
                ContentToken::BeginTextObject => {
                    in_text_object = true;
                    current_text_object = TextObjectContent {
                        positioned_text: vec![]
                    };
                },
                ContentToken::ShowTextString(text) => {
                    println!("\n\nGOT NON OBJECT TEXT: {}\n\n", text);
                },
                _ => {
                    println!("{:?}", token);
                }
            }
        }
    }

    // print!("{:?}", text_objects);

    text_objects
}


pub fn compile_grouped_text(object_contents: &[TextObjectContent]) {
    for content in object_contents {
        for text in &content.positioned_text {
            print!("{}", text.text);
        }
        println!();
    }
}