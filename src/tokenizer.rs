use std::{io::{prelude::*, SeekFrom}, str::FromStr};
use regex::Regex;
use log::{debug};

/*
    Notes: states for parsing list values and dictionary values are nearly identical
    except when a dictionary value is found, state needs to be popped and when
    list values are found they dont. And ']' is okay to find in the list value state
 */

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct PDFObjectHeader {
    pub object_number: u64,
    pub generation_number: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct XRefEntry {
    pub byte_offset: u64,
    pub generation_number: u64,
    pub free: bool
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct XRefHeader {
    pub first_object_number: u64,
    pub num_entries: u64
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct XRefSection {
    pub header: XRefHeader,
    pub entries: Vec<XRefEntry>
}

#[derive(Debug)]
pub enum PDFToken {
    Comment(String),
    ObjectHeader(PDFObjectHeader),
    ObjectEnd,
    ObjectReference(PDFObjectHeader),
    DictionaryStart,
    DictionaryEnd,
    Name(String),
    ArrayStart,
    ArrayEnd,
    StringBegin,
    String(String),
    StringEnd,
    HexString(Vec<u8>),
    Boolean(bool),
    Number(f64),
    StartXRef(u64),
    XRefSectionBegin,
    XRefSectionEnd,
    XRefSubSectionHeader(XRefHeader),
    XRefEntry(XRefEntry),
    Null,
    StreamBegin,
    StreamEnd,
    TrailerBegin,
    DocumentEnd
}

pub trait PDFTokenPatterns {
    fn is_positive_int(&self) -> bool;
    fn is_int(&self) -> bool;
    fn is_float(&self) -> bool;
    fn is_version(&self) -> bool;
    fn is_name(&self) -> bool;
    fn is_comment(&self) -> bool;
    fn is_object(&self) -> bool;
}

impl PDFTokenPatterns for String {
    fn is_positive_int(&self) -> bool {
        let positive_int_pattern: Regex = Regex::new(r"^\d+$").unwrap();
        positive_int_pattern.is_match(self.as_str())
    }

    fn is_object(&self) -> bool {
        let object_pattern: Regex = Regex::new(r"^\d+\s\d+\sobj$").unwrap();
        object_pattern.is_match(self.as_str())
    }

    fn is_int(&self) -> bool {
        Regex::new(r"^-?\d+$")
            .unwrap()
            .is_match(self.as_str())
    }

    fn is_float(&self) -> bool {
        Regex::new(r"^-?\d+(\.\d+)?$")
            .unwrap()
            .is_match(self.as_str())
    }

    fn is_version(&self) -> bool {
        Regex::new(r"^PDF-(\d\.\d)$")
            .unwrap()
            .is_match(self.as_str())
    }

    fn is_name(&self) -> bool {
        Regex::new(r"/.+")
            .unwrap()
            .is_match(self.as_str())
    }

    fn is_comment(&self) -> bool {
        self.starts_with("%")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenizerState {
    Start,
    Object,
    DictionaryKey,
    DictionaryValue,
    ListValue,
    Stream,
    StreamEnd,
    XRefSection,
    XRefEntry,
    DocumentEnd,
    Trailer
}

pub struct Tokenizer<T: Read + Seek> {
    state_stack: Vec<TokenizerState>,
    reader: T
}

pub trait PDFTokenize {
    fn next(&mut self) -> Result<PDFToken, String>;
    fn get_offset(&mut self) -> u64;
    fn get_stream(&mut self, num_bytes: usize) -> Vec<u8>;
    fn peak_next(&mut self) -> Result<PDFToken, String>;
    fn peak_multiple(&mut self, num_tokens: u32) -> Result<Vec<PDFToken>, String>;
    fn get_xref_table(&mut self, num_entries: u64) -> Result<Vec<XRefEntry>, String>;
}


impl<T: Read + Seek> Tokenizer<T> {
    pub fn new(reader: T) -> Self {
        Tokenizer {
            state_stack: vec![TokenizerState::Start],
            reader: reader
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let mut next_byte: [u8; 1] = [0];
        match self.reader.read(&mut next_byte).unwrap() {
            0 => None,
            _ => Some(char::from_u32(next_byte[0].into()).unwrap())
        }
    }

    fn read_until(&mut self, until_chars: Vec<char>, seek_back: bool) -> String {
        let mut result = String::new();
        while let Some(next_char) = self.next_char() {
            if until_chars.contains(&next_char) {
                if seek_back {
                    self.reader.seek(SeekFrom::Current(-1)).unwrap();
                }
                break;
            }
            result.push(next_char);
        }
        result
    }

    fn read_number(&mut self) -> Result<f64, <f64 as FromStr>::Err> {
        self.consume_whitespace();
        self.read_until(vec![' ', '>', ']', '[', '/', '\n', '\r'], true).parse::<f64>()
    }

    fn consume_whitespace(&mut self) {
        loop {
            match self.next_char().unwrap() {
                ' ' | '\n' | '\r' => continue,
                _ => {
                    self.reader.seek(SeekFrom::Current(-1)).unwrap();
                    break;
                }
            }
        }
    }

    fn read_comment(&mut self) -> String {
        self.read_until(vec!['\n','\r'], false)
    }

    fn read_n_chars(&mut self, num_chars: u32) -> String {
        let mut result = String::new();
        for _ in 0..num_chars {
            result.push(self.next_char().unwrap());
        }
        result
    }

    fn read_object_header(&mut self) -> Result<PDFObjectHeader, String> {
        let object_number = self.read_until(vec![' '], false).parse::<u64>().unwrap();
        let generation_number = self.read_until(vec![' '], false).parse::<u64>().unwrap();
        
        match  self.read_n_chars(3).as_str() {
            "obj" => Ok(PDFObjectHeader {
                object_number,
                generation_number
            }),
            other => Err(format!("Unexpected value {} while reading object header", other)),
        }
    }

    fn read_object_reference(&mut self) -> Result<PDFToken, String> {
        let object_number = match self.read_until(vec![' '], false).parse::<u64>() {
            Ok(value) => value,
            Err(err) => {
                return Err(err.to_string());
            }
        };
        
        let generation_number = match self.read_until(vec![' '], false).parse::<u64>() {
            Ok(value) => value,
            Err(err) => {
                return Err(err.to_string());
            }
        };

        match self.next_char().unwrap() {
            'R' => {
                Ok(PDFToken::ObjectReference(PDFObjectHeader {
                    object_number,
                    generation_number
                }))
            },
            other => {
                Err(format!("Found unexpected char '{other}' while reading object reference"))
            }
        }
    }

    fn get_state(&mut self) -> TokenizerState {
        self.state_stack.last().unwrap().clone()
    }

    fn push_state(&mut self, state: TokenizerState) {
        debug!("Pushing state: {:?}", state);
        self.state_stack.push(state);
    }

    fn pop_state(&mut self) -> TokenizerState {
        let popped_state = self.state_stack.pop().unwrap();
        debug!("Popping state: {:?}", popped_state);
        if self.state_stack.is_empty() {
            debug!("Current state: {:?}", self.state_stack.last());
        } else {
            debug!("Current state: {:?}", self.state_stack.last().unwrap());
        }
        popped_state
    }

    fn read_literal_string(&mut self) -> Result<String, String> {
        let mut parenthesis_stack: Vec<char> = vec![];
        let mut literal_string = String::new();

        loop  {
            let next_char = self.next_char().unwrap();
            match next_char {
                '(' => {
                    if !parenthesis_stack.is_empty() {
                        literal_string.push(next_char);
                    }
                    parenthesis_stack.push(next_char);
                },
                ')' => {
                    parenthesis_stack.pop();
                    if parenthesis_stack.is_empty() {
                        break;
                    }
                    literal_string.push(next_char);
                },
                '\\' => {
                    let next_char = self.next_char().unwrap();
                    match next_char {
                        '\\' | '(' | ')' => {
                            literal_string.push(next_char);
                        },
                        'r' => {
                            literal_string.push('\r');
                        },
                        'n' => {
                            literal_string.push('\n');
                        },
                        'b' => {
                            // Rust does not recognize \b as a valid escape sequence :(
                            literal_string.push(char::from_u32(0x08).unwrap());
                        },
                        't' => {
                            literal_string.push('\t');
                        },
                        'f' => {
                            // Rust does not recognize \b as a valid escape sequence :(
                            literal_string.push(char::from_u32(0x0C).unwrap());
                        },
                        '0'..='9' => {
                            // Octal character code
                            let mut octal_string = next_char.to_string();
                            octal_string.push(self.next_char().unwrap());
                            octal_string.push(self.next_char().unwrap());
                            let char_code = u32::from_str_radix(octal_string.as_str(), 8).unwrap();
                            literal_string.push(char::from_u32(char_code).unwrap());
                        },
                        unhandled => {
                            return Err(format!("Unhandled escaped character '{unhandled}' in literal string"));
                        }
                    }
                },
                _ => {
                    literal_string.push(next_char);
                }
            }
        }

        Ok(literal_string)
    }

    fn hex_string_to_bytes(&mut self, hex_string: String) -> Result<Vec<u8>, String> {
        let mut hex_string = hex_string;

        if hex_string.len() % 2 == 1 {
            /*
             * 7.3.4.3 Hexadecimal Strings
             * If the final digit of a hexadecimal string is missing—that is, if there
             * is an odd number of digits—the final digit shall be assumed to be 0.
             */
            hex_string.push('0');
        }

        let mut bytes: Vec<u8> = vec![];

        let mut hex_string = hex_string.clone();
        while !hex_string.is_empty() {
            let mut hex_byte = hex_string.pop().unwrap().to_string();
            hex_byte.push(hex_string.pop().unwrap());
            bytes.push(u8::from_str_radix(hex_byte.as_str(), 16).unwrap())
        }

        Ok(bytes)
    }
}

impl<T: Read + Seek> PDFTokenize for Tokenizer<T> {
    fn next(&mut self) -> Result<PDFToken, String> {

        let state = self.state_stack.last().expect("State stack is empty!").to_owned();
        loop {
            match state {
                TokenizerState::Start => match self.next_char().unwrap() {
                    ' ' | '\n' | '\r' => continue,
                    '%' => {
                        let comment = self.read_comment().trim().to_string();
                        if comment == "%EOF" {
                            self.state_stack.pop();
                            self.state_stack.push(TokenizerState::DocumentEnd);
                            return Ok(PDFToken::DocumentEnd);
                        }
                        return Ok(PDFToken::Comment(comment))
                    },
                    '1'..='9' => {
                        self.pop_state();
                        self.push_state(TokenizerState::Object);
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        return match self.read_object_header() {
                            Ok(object_header) => Ok(PDFToken::ObjectHeader(object_header)),
                            Err(err) => Err(err)
                        }
                    },
                    's' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![' ', '\n', '\r'], false).as_str() {
                            "startxref" => {
                                let xref_offset = self.read_number().unwrap();
                                return Ok(PDFToken::StartXRef(xref_offset as u64));
                            }
                            other => panic!("Found unexpected keyword '{other}' while reading object")
                        }
                    },
                    'x' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![' ', '\n', '\r'], false).as_str() {
                            "xref" => {
                                self.push_state(TokenizerState::XRefSection);
                                return Ok(PDFToken::XRefSectionBegin);
                            }
                            other => panic!("Found unexpected keyword '{other}' while reading object")
                        }
                    },
                    't' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![' ', '\n', '\r'], false).as_str() {
                            "trailer" => {
                                self.push_state(TokenizerState::Trailer);
                                return Ok(PDFToken::TrailerBegin);
                            }
                            other => panic!("Found unexpected keyword '{other}' while reading object")
                        }
                    }
                    unhandled_char => todo!("Top level char '{unhandled_char}' not handled")
                }
                TokenizerState::DocumentEnd => {
                    return Err("End of document reached!".to_owned());
                }
                TokenizerState::Object => match self.next_char().unwrap() {
                    ' ' | '\n' | '\r' => continue,
                    '<' => {
                        let next = self.next_char().unwrap();
                        if next == '<' {
                            self.push_state(TokenizerState::DictionaryKey);
                            return Ok(PDFToken::DictionaryStart);
                        }
                        return Err(format!("Unexpected character `{next}` while parsing dictionary start"));
                    },
                    '[' => {
                        self.push_state(TokenizerState::ListValue);
                        return Ok(PDFToken::ArrayStart);
                    },
                    's' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![' ', '\n', '\r'], false).as_str() {
                            "stream" => {
                                self.consume_whitespace();
                                self.push_state(TokenizerState::Stream);
                                return Ok(PDFToken::StreamBegin);
                            }
                            other => panic!("Found unexpected keyword '{other}' while reading object")
                        }
                    },
                    'e' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![' ', '\n', '\r'], false).trim() {
                            "endobj" => {
                                self.pop_state();
                                self.push_state(TokenizerState::Start);
                                return Ok(PDFToken::ObjectEnd);
                            }
                            other => panic!("Found unexpected keyword '{other}' while reading object")
                        }
                    },
                    '(' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        return Ok(PDFToken::String(self.read_literal_string()?));
                    },
                    unhandled_char => panic!("Unhandled char {unhandled_char} while looking for object")
                },
                TokenizerState::DictionaryKey => match self.next_char().unwrap() {
                    ' ' | '\n' | '\r' => continue,
                    '/' => {
                        let name = self.read_until(vec![' ','/','<','[','(', '\r', '\n'], true);
                        self.push_state(TokenizerState::DictionaryValue);
                        return Ok(PDFToken::Name(name));
                    },
                    '>' => {
                        match self.next_char().unwrap() {
                            '>' => {
                                self.pop_state();
                                if self.state_stack.last().unwrap().clone() == TokenizerState::DictionaryValue {
                                    // If this dictionary exists in another dictionary, then popping the 
                                    self.pop_state();
                                }
                                return Ok(PDFToken::DictionaryEnd)
                            },
                            other => return Err(format!("Found unexpected character '{other}' while parsing dictionary"))
                        }
                    },
                    unhandled_char => panic!("Unhandled char '{unhandled_char}' while looking for dictionary key")
                },
                TokenizerState::DictionaryValue => match self.next_char().unwrap() {
                    ' ' | '\n' | '\r' => continue,
                    '[' => {
                        self.pop_state();
                        self.push_state(TokenizerState::ListValue);
                        return Ok(PDFToken::ArrayStart);
                    },
                    '/' => {
                        let name = self.read_until(vec![' ',']','/','\n', '>'], true);
                        self.pop_state();
                        return Ok(PDFToken::Name(name));
                    },
                    't' | 'f' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec!['\n','/','>'], true).trim() {
                            "true" => {
                                self.pop_state();
                                return Ok(PDFToken::Boolean(true));
                            },
                            "false" => {
                                self.pop_state();
                                return Ok(PDFToken::Boolean(false));
                            },
                            token => {
                                panic!("Unexpected value '{token}' while parsing dictionary value")
                            }
                        }

                    },
                    '0'..='9' | '-' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        let offset = self.reader.stream_position().unwrap();
                        let object_reference = self.read_object_reference();
                        if object_reference.is_err() {
                            self.reader.seek(SeekFrom::Start(offset)).unwrap();
                            self.pop_state();
                            return Ok(PDFToken::Number(self.read_number().unwrap()));
                        } else {
                            self.pop_state();
                            return object_reference;
                        }
                    },
                    '<' => {
                        match self.next_char().unwrap() {
                            '<' => {
                                self.push_state(TokenizerState::DictionaryKey);
                                return Ok(PDFToken::DictionaryStart);
                            },
                            'a'..='z' | 'A'..='Z' | '0'..='9' => {
                                self.pop_state();
                                let hex_string = self.read_until(vec!['>'], false);
                                let bytes= self.hex_string_to_bytes(hex_string);
                                return Ok(PDFToken::HexString(bytes.unwrap()));
                            },
                            other => {
                                return Err(format!("Unexpected character `{other}` while parsing dictionary/hex-string start. State: {:?}", state));
                            }
                        }
                    },
                    'n' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![']', ' ', '\n'], true).as_str() {
                            "null" => {
                                return Ok(PDFToken::Null);
                            },
                            unhandled => {
                                return Err(format!("Unexpected string '{unhandled}' while looking for null"));
                            }
                        }
                    },
                    '(' => {
                        self.pop_state();
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        return Ok(PDFToken::String(self.read_literal_string()?));
                    },
                    unhandled_char => return Err(format!("Unhandled char '{unhandled_char}' while looking for dictionary value"))
                },
                TokenizerState::ListValue => match self.next_char().unwrap() {
                    ' ' | '\n' | '\r' => continue,
                    ']' => {
                        // Pop List State
                        self.pop_state();
                        return Ok(PDFToken::ArrayEnd);
                    },
                    '[' => {
                        self.push_state(TokenizerState::ListValue);
                        return Ok(PDFToken::ArrayStart);
                    },
                    '/' => {
                        let name = self.read_until(vec![' ',']'], true);
                        return Ok(PDFToken::Name(name));
                    },
                    '0'..='9' | '-' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        let offset: u64 = self.reader.stream_position().unwrap();
                        let object_reference = self.read_object_reference();
                        if object_reference.is_err() {
                            self.reader.seek(SeekFrom::Start(offset)).unwrap();
                            return Ok(PDFToken::Number(self.read_number().unwrap()));
                        } else {
                            return object_reference;
                        }
                    },
                    '<' => {
                        match self.next_char().unwrap() {
                            '<' => {
                                self.push_state(TokenizerState::DictionaryKey);
                                return Ok(PDFToken::DictionaryStart);
                            },
                            'a'..='z' | 'A'..='Z' | '0'..='9' => {
                                // self.reader.seek(SeekFrom::Current(-1)).unwrap();
                                let hex_string = self.read_until(vec!['>'], false);
                                let bytes= self.hex_string_to_bytes(hex_string);
                                return Ok(PDFToken::HexString(bytes.unwrap()));
                            },
                            other => {
                                return Err(format!("Unexpected character `{other}` while parsing dictionary/hex-string start. State: {:?}", state));
                            }
                        }
                    },
                    'n' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![']', ' ', '\n'], true).as_str() {
                            "null" => {
                                return Ok(PDFToken::Null);
                            },
                            unhandled => {
                                return Err(format!("Unexpected string '{unhandled}' while looking for null"));
                            }
                        }
                    },
                    't' | 'f' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        match self.read_until(vec![']', ' ', '>', '\n'], true).as_str() {
                            "true" => {
                                return Ok(PDFToken::Boolean(true));
                            },
                            "false" => {
                                return Ok(PDFToken::Boolean(false));
                            },
                            unhandled => {
                                return Err(format!("Unexpected string '{unhandled}' while looking for null"));
                            }
                        }
                    },
                    '(' => {
                        self.reader.seek(SeekFrom::Current(-1)).unwrap();
                        return Ok(PDFToken::String(self.read_literal_string()?));
                    },
                    unhandled_char => return Err(format!("Unhandled char '{unhandled_char}' while looking for list value"))
                },
                TokenizerState::Stream => {
                    return Err("next() called in Stream".to_string());
                },
                TokenizerState::StreamEnd => {
                    match self.next_char().unwrap() {
                        ' ' | '\n' | '\r' => continue,
                        'e' => {
                            self.reader.seek(SeekFrom::Current(-1)).unwrap();
                            match self.read_until(vec![' ', '\n', '\r'], false).as_str() {
                                "endstream" => {
                                    self.pop_state();
                                    return Ok(PDFToken::StreamEnd);
                                },
                                other => panic!("Found unexpected keyword '{other}' while reading object")
                            }
                        },
                        unhandled_char => return Err(format!("Unhandled char '{unhandled_char}' expected 'streamend'"))
                    }
                },
                TokenizerState::XRefSection => {
                    let first_object_number = self.read_number().unwrap() as u64;
                    self.next_char();
                    let num_entries = self.read_number().unwrap() as u64;
                    self.read_until(vec!['\n'], false);
                    self.push_state(TokenizerState::XRefEntry);
                    return Ok(PDFToken::XRefSubSectionHeader(XRefHeader { first_object_number, num_entries }));
                },
                TokenizerState::XRefEntry => {
                    let byte_offset = self.read_number().unwrap() as u64;
                    self.next_char();
                    let generation_number = self.read_number().unwrap() as u64;

                    let free = match self.read_until(vec!['\n'], false).trim() {
                        "f" => true,
                        "n" => false,
                        other => {
                            return Err(format!("Unexpected value: '{other}' while parsing xref entry"));
                        }
                    };

                    return Ok(PDFToken::XRefEntry(XRefEntry {
                        byte_offset,
                        generation_number,
                        free
                    }));
                }
                TokenizerState::Trailer => {
                    loop {
                        match self.next_char().unwrap() {
                            '\n' => {},
                            '<' => {
                                let next = self.next_char().unwrap();
                                if next == '<' {
                                    self.pop_state();
                                    self.push_state(TokenizerState::DictionaryKey);
                                    return Ok(PDFToken::DictionaryStart);
                                }
                                return Err(format!("Unexpected character `{next}` while parsing trailer dictionary"));
                            },
                            other => {
                                return Err(format!("Unexpected character `{other}` while looking for trailer dictionary"));
                            }
                        }
                    }
                }
            }
        }
    }

    fn peak_next(&mut self) -> Result<PDFToken, String> {
        let state_stack_before_peak = self.state_stack.clone();
        let offset_before_peak = self.reader.stream_position().unwrap();
        let next_token = self.next();
        self.state_stack = state_stack_before_peak;
        debug!("Restoring state stack after peak: {:?}", self.state_stack.clone());
        self.reader.seek(SeekFrom::Start(offset_before_peak)).unwrap();
        next_token
    }

    fn peak_multiple(&mut self, num_tokens: u32) -> Result<Vec<PDFToken>, String> {
        let offset_before_peak = self.reader.stream_position().unwrap();
        let state_stack_before_peak = self.state_stack.clone();

        let mut tokens = Vec::<PDFToken>::with_capacity(num_tokens as usize);
        for _ in 1..num_tokens {
            match self.next() {
                Ok(token) => {
                    tokens.push(token)
                },
                Err(err) => {
                    return Err(err)
                }
            }
        }
        self.state_stack = state_stack_before_peak;
        println!("Restoring state stack after peak multiple: {:?}", self.state_stack.clone());
        self.reader.seek(SeekFrom::Start(offset_before_peak));
        Ok(tokens)
    }

    fn get_offset(&mut self) -> u64 {
        self.reader.stream_position().unwrap()
    }

    fn get_stream(&mut self, num_bytes: usize) -> Vec<u8> {
        let mut bytes = vec![0; num_bytes];

        self.reader.read_exact(&mut bytes).unwrap();

        self.pop_state();
        self.push_state(TokenizerState::StreamEnd);

        bytes
    }

    fn get_xref_table(&mut self, num_entries: u64) -> Result<Vec<XRefEntry>, String> {
        assert!(self.get_state() == TokenizerState::XRefEntry);
        let mut entries: Vec<XRefEntry> = vec![];

        for _ in 0..num_entries {
            let token = self.next();
            debug!("{:?}", token);
            let entry = match token  {
                Ok(PDFToken::XRefEntry(entry)) => entry,
                Err(err) => {
                    return Err(err);
                },
                other_token => {
                    return Err(format!("Unexpected token: {:?} while reading xref table entry", other_token));
                },
            };
            entries.push(entry);
        }

        self.state_stack.pop();
        self.state_stack.pop();

        Ok(entries)
    }
}