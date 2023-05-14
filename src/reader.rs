use log::debug;

use crate::pdf::{PDFDictionary};
use crate::tokenizer::{PDFTokenize, PDFToken, PDFObjectHeader, XRefSection, XRefEntry};

use super::tokenizer::{PDFTokenPatterns};
use super::pdf::{PDF, PDFObject, PDFValue};

pub struct Reader<T: PDFTokenize> {
    pdf: PDF,
    tokenizer: T
} 

impl<T: PDFTokenize> Reader<T> {
    pub fn new(tokenizer: T) -> Self {
        Self {
            tokenizer,
            pdf: Default::default(),
        }
    }

    pub fn read(&mut self) {
        self.parse();
    }

    pub fn parse(&mut self) {
        loop {
            let current_offset = self.tokenizer.get_offset();
            let token = self.tokenizer.next();
            debug!("{:?}", token.as_ref());

            match token.as_ref() {
                Ok(PDFToken::Comment(comment)) => {
                    if comment.is_version() {
                        self.pdf.version = Some(comment.to_string());
                        println!("version: {}", self.pdf.version.as_ref().unwrap().to_owned());
                    }
                },
                Ok(PDFToken::ObjectHeader(object_header)) => {
                    let pdf_object = self.parse_object(current_offset, object_header).unwrap();
                    self.pdf.objects.insert(pdf_object.header, pdf_object);
                },
                Ok(PDFToken::StartXRef(xref_offset)) => {
                    self.pdf.startxref = Some(*xref_offset);
                },
                Ok(PDFToken::DocumentEnd) => {
                    break;
                },
                Ok(PDFToken::XRefSectionBegin) => {
                    self.parse_xref().unwrap();
                },
                Ok(PDFToken::TrailerBegin) => {
                    match self.parse_value() {
                        Ok(PDFValue::Dictionary(trailer_dictionary)) => {
                            self.pdf.trailer = Some(trailer_dictionary);
                        },
                        Ok(other) => {
                            panic!("Unexpected token '{:?}' while looking for trailer dictionary", other);
                        },
                        Err(err) => {
                            panic!("Trailer parse error: {err}");
                        }
                    }
                },
                Ok(something) => {
                    panic!("Unexpected token {:?}", something);
                },
                Err(err) => {
                    panic!("{err}");
                }
            }
        }
    }

    fn parse_xref(&mut self) -> Result<XRefSection, String> {
        let token = self.tokenizer.next();
        debug!("{:?}", token.as_ref());


        let header = match token {
            Ok(PDFToken::XRefSubSectionHeader(header)) => {
                header
            },
            Err(err) => {
                return Err(err);
            }
            other_token => {
                return Err(format!("Unexpected token: {:?} while reading xref table", other_token));
            },
        };

        let entries: Vec<XRefEntry> = self.tokenizer.get_xref_table(header.num_entries).unwrap();

        Ok(XRefSection {
            header,
            entries
        })
    }

    fn parse_array(&mut self) -> Result<PDFValue, String> {
        let mut values: Vec<PDFValue> = vec![];

        loop {
            let next_token = self.tokenizer.peak_next();
            match next_token {
                Ok(PDFToken::ArrayEnd) => {
                    // Consume array end token
                    debug!("{:?}", next_token.as_ref());
                    self.tokenizer.next().unwrap();
                    break;
                },
                Ok(_) => {
                    values.push(self.parse_value().unwrap());
                },
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(PDFValue::Array(values))
    }

    fn parse_dictionary(&mut self) -> Result<PDFDictionary, String> {
        let mut dictionary = PDFDictionary::new();

        loop {
            let token = self.tokenizer.next();
            debug!("{:?}", token.as_ref());
            let key = match token {
                Ok(PDFToken::DictionaryEnd) => {
                    break
                },
                Ok(PDFToken::Name(name)) => name,
                Ok(token) => {
                    return Err(format!("Got unexpected token {:?} while looking for dictionary key", token));
                },
                Err(err) => {
                    return Err(err);
                }
            };


            let value = self.parse_value().unwrap();

            dictionary.insert(key, value);
        }

        Ok(dictionary)
    }

    fn parse_stream(&mut self, stream_dictionary: PDFDictionary) -> Result<PDFValue, String> {
        let length = match stream_dictionary.get("Length") {
            Some(PDFValue::Number(number)) => number,
            Some(_) => {
                return Err("Stream dictionary has a Length that is not a number".to_string())
            },
            None => {
                return Err("Stream dictionary has no Length member".to_string());
            }
        };

        let bytes = self.tokenizer.get_stream(*length as usize);

        let next_token = self.tokenizer.next();
        debug!("{:?}", next_token.as_ref());

        match next_token? {
            PDFToken::StreamEnd => Ok(PDFValue::Bytes(bytes)),
            token => Err(format!("Unexpected token {:?} while parsing stream", token))
        }
    }

    fn parse_value(&mut self) -> Result<PDFValue, String> {
        let token = self.tokenizer.next();
        debug!("{:?}", token.as_ref());
        match token {
            Ok(PDFToken::ArrayStart) => {
                self.parse_array()
            },
            Ok(PDFToken::DictionaryStart) => {
                let dictionary = self.parse_dictionary().unwrap();
                match self.tokenizer.peak_next() {
                    Ok(PDFToken::StreamBegin) => {
                        debug!("{:?}", self.tokenizer.next());
                        self.parse_stream(dictionary)
                    },
                    Ok(_) => Ok(PDFValue::Dictionary(dictionary)),
                    Err(err) => Err(err)
                }
            },
            Ok(PDFToken::Name(name)) => {
                Ok(PDFValue::String(name))
            },
            Ok(PDFToken::String(string_token)) => {
                Ok(PDFValue::String(string_token))
            },
            Ok(PDFToken::Number(number)) => {
                Ok(PDFValue::Number(number))
            },
            Ok(PDFToken::Boolean(value)) => {
                Ok(PDFValue::Boolean(value))
            },
            Ok(PDFToken::ObjectReference(object_header)) => {
                Ok(PDFValue::ObjectReference(object_header))
            },
            Ok(PDFToken::Null) => {
                Ok(PDFValue::Null)
            },
            Ok(PDFToken::HexString(bytes)) => {
                Ok(PDFValue::Bytes(bytes))
            },
            Ok(token) => {
                todo!("Could not parse {:?}", token)
            },
            Err(err) => Err(err)
        }
    }

    fn parse_object(&mut self, offset: u64, header: &PDFObjectHeader) -> Result<PDFObject, String> {
        let value = self.parse_value().unwrap();

        let next_token = self.tokenizer.next();
        debug!("{:?}", next_token.as_ref());

        match next_token? {
            PDFToken::ObjectEnd => Ok(PDFObject {
                header: *header,
                value,
                offset
            }),
            token => Err(format!("Unexpected token {:?} while parsing object", token))
        }
    }
}
