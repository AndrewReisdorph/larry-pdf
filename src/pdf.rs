
use std::collections::HashMap;
use std::option::Option;

use flate2::Decompress;

use crate::tokenizer::{PDFObjectHeader, XRefSection};
use crate::page::{PDFPage};

pub type PDFDictionary = HashMap<String, PDFValue>;

#[derive(Debug, PartialEq, Clone)]
pub struct PDFStream {
    pub dictionary: PDFDictionary,
    pub bytes: Vec<u8>
}

impl PDFStream {
    pub fn decompress(&self) -> Vec<u8> {
        let mut decompress = Decompress::new(true);
        let mut decompressed_bytes: Vec<u8> = Vec::with_capacity(self.bytes.len() * 3);
        decompress.decompress_vec(
            &self.bytes,
            &mut decompressed_bytes,
            flate2::FlushDecompress::Sync).unwrap();
        decompressed_bytes
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum PDFValue {
    Dictionary(PDFDictionary),
    Boolean(bool),
    Array(Vec<PDFValue>),
    String(String),
    ObjectReference(PDFObjectHeader),
    Number(f64),
    Name(String),
    Stream(PDFStream),
    Bytes(Vec<u8>),
    Null
}

impl PDFValue {
    pub fn object_reference(&self) -> &PDFObjectHeader {
        if let PDFValue::ObjectReference(object_reference) = self {
            object_reference
        } else {
            panic!("Value is not ObjectReference")
        }
    }

    pub fn dictionary(&self) -> Result<&PDFDictionary, String> {
        match self {
            PDFValue::Dictionary(dictionary) => Ok(dictionary),
            _ => Err("Value is not Dictionary".to_string())
        }
    }

    pub fn stream(&self) -> Result<&PDFStream, String> {
        match self {
            PDFValue::Stream(stream) => Ok(stream),
            _ => Err("Value is not Stream".to_string())
        }
    }

    pub fn array(&self) -> &Vec<PDFValue> {
        if let PDFValue::Array(array) = self {
            array
        } else {
            panic!("Value is not Array")
        }
    }
}

#[derive(Debug, Clone)]
pub struct PDFObject {
    pub header: PDFObjectHeader,
    pub value: PDFValue,
    pub offset: u64
}

#[derive(Default)]
pub struct PDF {
    pub version: Option<String>,
    pub objects: HashMap<PDFObjectHeader, PDFObject>,
    pub startxref: Option<u64>,
    pub root: Option<PDFObject>,
    pub trailer: Option<PDFDictionary>,
    pub xref_table: Option<XRefSection>,
    pub pages: Vec<PDFPage>,
}
