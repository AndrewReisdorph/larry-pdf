use std::borrow::Borrow;
use std::io::{Cursor, Read};
use std::thread::panicking;

use log::debug;

use flate2::Decompress;

use crate::page::PDFPage;
use crate::pdf::{PDFDictionary, PDFStream};
use crate::tokenizer::{PDFTokenize, PDFToken, PDFObjectHeader, XRefSection, XRefEntry, XRefStreamFreeObject, XRefStreamUncompressedObject, XRefStreamCompressedObject};

use super::tokenizer::{PDFTokenPatterns};
use super::pdf::{PDF, PDFObject, PDFValue};

pub struct Reader<T: PDFTokenize> {
    pdf: PDF,
    tokenizer: T
}

trait ReadU64 {
    fn read_u64(&mut self, num_bytes: u8) -> u64;
}

impl ReadU64 for Cursor<Vec<u8>> {
    fn read_u64(&mut self, num_bytes: u8) -> u64 {
        assert!(num_bytes <= 8, "Width exceeds size of u64");
        let mut buf: [u8; 8] = [0; 8];
        let mut source_bytes_buf: Vec<u8> = vec![0; num_bytes as usize];
        self.read_exact(&mut source_bytes_buf).unwrap();
        for i in 0..num_bytes {
            buf[7 - i as usize] = source_bytes_buf[(num_bytes - i - 1) as usize];
        }

        u64::from_be_bytes(buf)
    }
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
        self.build_tree();
    }

    fn parse_xref_stream(&mut self, widths: Vec<u64>, bytes: Vec<u8>) -> Vec<XRefEntry> {
        assert!(widths.first() == Some(&1), "First width was not zero!");
        let second_field_width = *widths.get(1).unwrap_or_else( || panic!("Not enough values in widths array: {:?}", widths));
        let third_field_width = *widths.get(2).unwrap_or_else( || panic!("Not enough values in widths array: {:?}", widths));

        let mut entries: Vec<XRefEntry> = vec![];

        let mut cursor = Cursor::new(bytes);
        let mut next_byte: [u8; 1] = [0];

        while cursor.read_exact(&mut next_byte).is_ok() {
            let entry_type = next_byte[0];
            match entry_type {
                0 => {
                    let object_number_of_next_free_object = cursor.read_u64(second_field_width as u8);
                    let generation_number_for_next_object_use = cursor.read_u64(third_field_width as u8);
                    entries.push(XRefEntry::Free(XRefStreamFreeObject {
                        object_number_of_next_free_object,
                        generation_number_for_next_object_use
                    }));
                },
                1 => {
                    let byte_offset = cursor.read_u64(second_field_width as u8);
                    let generation_number = cursor.read_u64(third_field_width as u8);
                    entries.push(XRefEntry::Uncompressed(XRefStreamUncompressedObject {
                        byte_offset,
                        generation_number
                    }));
                },
                2 => {
                    let object_number_of_parent_stream = cursor.read_u64(second_field_width as u8);
                    let index_in_stream = cursor.read_u64(third_field_width as u8);
                    entries.push(XRefEntry::Compressed(XRefStreamCompressedObject {
                        object_number_of_parent_stream,
                        index_in_stream
                    }));
                },
                _ => {
                    panic!("Unsupported xref entry type {entry_type}");
                }
            }
        }

        entries
    }

    fn get_object_at_offset(&mut self, offset: u64) -> Option<PDFObject> {
        for object in self.pdf.objects.values() {
            if object.offset == offset {
                return Some(object.clone());
            }
        }
        None
    }

    fn get_object_by_reference(&mut self, reference: &PDFObjectHeader) -> Option<PDFObject> {
        self.pdf.objects.get(reference).cloned()
    }

    fn get_root_object(&mut self) -> Result<PDFObject, String> {
        if let Some(trailer) = &self.pdf.trailer {
            if let Some(PDFValue::Dictionary(trailer_dict)) = trailer.get("Root") {
                debug!("Trailer: {:?}", trailer_dict);
            }
        } else if let Some(startxref) = self.pdf.startxref {
            debug!("StartXRef: {:?}", startxref);

            if let PDFValue::Stream(stream) = self.get_object_at_offset(startxref).unwrap().value {
                let stream_length = if let PDFValue::Number(length) = stream.dictionary.get("Length").expect("XRef stream dictionary has no Length member") {
                    length
                } else {
                    panic!("XRef stream length cannot be converted from {:?}", stream.dictionary.get("Length"));
                };

                let width: &PDFValue = stream.dictionary.get("W").expect("No 'W' entry in xref stream dictionary");
                let mut width_vector: Vec<u64> = vec![];
                if let PDFValue::Array(width_array) = width {
                    for val in width_array.iter() {
                        if let PDFValue::Number(val) = val {
                            width_vector.push(*val as u64);
                        }
                    }
                }
                let xref_size = if let PDFValue::Number(xref_size) = stream.dictionary.get("Size").expect("XRef stream dictionary has no Size member") {
                    xref_size
                } else {
                    panic!("XRef size cannot be converted from {:?}", stream.dictionary.get("Size"));
                };

                let mut decompressed_bytes = stream.decompress();

                self.pdf.xref_table = Some(XRefSection {
                    header: None,
                    entries: self.parse_xref_stream(width_vector,decompressed_bytes)
                });

                let root: &PDFValue = stream.dictionary.get("Root").expect("No 'Root' entry in xref stream dictionary");
                match root {
                    PDFValue::ObjectReference(object_ref) => {
                        return Ok(self.get_object_by_reference(object_ref).expect("Root object not found"));
                    }
                    _ => panic!("Root object was not object reference")
                }
            } else {
                panic!("No stream object at startxref location: {startxref}")
            }
        }

        Err("".to_string())
    }

    fn get_pages_dict(&mut self, root: &PDFObject) -> Result<PDFDictionary, String> {
        let pages_obj_ref = root
            .value
            .dictionary()
            .unwrap()
            .get("Pages")
            .expect("Root dictionary has no pages member")
            .object_reference();

        Ok(self
            .get_object_by_reference(pages_obj_ref)
            .expect("Pages dictionary object not found")
            .value
            .dictionary()
            .unwrap()
            .clone()
        )
    }

    fn read_pages(&mut self, pages_dict: &PDFDictionary) -> Result<Vec<PDFPage>, String> {
        let mut pages: Vec<PDFPage> = vec![];

        let kids = pages_dict
            .get("Kids")
            .expect("Pages dict has no kids entry")
            .array();

        for kid in kids.iter() {
            debug!("kid: {:?}", kid);
            let object: PDFObject = self.get_object_by_reference(kid.object_reference()).expect("Page object not found");
            // debug!("kid object: {:?}", object);
            let page_dict = object.value.dictionary().unwrap();

            let contents_obj = match page_dict.get("Contents") {
                Some(PDFValue::ObjectReference(object_header)) => {
                    self.get_object_by_reference(object_header).unwrap()
                },
                Some(_) => {
                    return Err("Page dict has no 'Contents' entry".to_string());
                },
                None => {
                    return Err("Page dict has no 'Contents' entry".to_string());
                },
            };

            pages.push(PDFPage { object, contents: contents_obj });
        }

        Ok(pages)
    }

    fn build_tree(&mut self) {
        let root = self.get_root_object().unwrap();
        debug!("root object: {:?}", root);
        let pages_dict = self.get_pages_dict(&root).unwrap();
        debug!("pages_dict {:?}", pages_dict);
        self.pdf.pages = self.read_pages(&pages_dict).unwrap();

        let mut temp = 0;

        for page in self.pdf.pages.iter() {
            println!("==========================================");
            page.get_text(temp);
            temp += 1;
        }

        // panic!();
        // debug!("pages: {:?}", self.pdf.pages);
    }

    fn parse(&mut self) {
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
            header: Some(header),
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
            PDFToken::StreamEnd => Ok(PDFValue::Stream( PDFStream {bytes, dictionary: stream_dictionary})),
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
