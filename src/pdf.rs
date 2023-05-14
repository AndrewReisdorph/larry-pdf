
use std::collections::HashMap;
use std::option::Option;

use crate::tokenizer::PDFObjectHeader;

pub type PDFDictionary = HashMap<String, PDFValue>;

pub struct PDFPage {

}

#[derive(Debug)]
pub enum PDFValue {
    Dictionary(PDFDictionary),
    Boolean(bool),
    Array(Vec<PDFValue>),
    String(String),
    ObjectReference(PDFObjectHeader),
    Number(f64),
    Name(String),
    Bytes(Vec<u8>),
    Null
}

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
    pub trailer: Option<PDFDictionary>,
    pub pages: Vec<PDFPage>,
}

// impl Default for PDF {
//     fn default() -> Self {
//         PDF {
//             version: None,
//             objects: HashMap::new(),
//             startxref: None,
//             trailer: None,
//             pages: Vec::new(),
//         }
//     }
// }
