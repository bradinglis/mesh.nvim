#![feature(try_trait_v2)]
#![feature(iter_intersperse)]

pub mod annotations;

use std::path::Path;

use thiserror::Error;

use crate::annotations::display_outline_item;

pub trait ToNoteId {
    fn to_note_id(self) -> String;
}

impl ToNoteId for String {
    fn to_note_id(self) -> String {
        self.split_whitespace()
            .filter_map(|word| {
                if word.starts_with(|ch: char| ch.is_lowercase()) {
                    None
                } else {
                    Some(
                        word.to_lowercase()
                            .chars()
                            .filter(|y| *y == '-' || y.is_ascii_alphanumeric())
                            .collect(),
                    )
                }
            })
            .collect::<Vec<String>>()
            .join("-")
    }
}

pub type Result<T> = std::result::Result<T, MyError>;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Pdf error: {0}")]
    Pdf(#[from] pdf_oxide::Error),
    #[error("Message: {0}")]
    String(String),
    #[error("File error: {0}")]
    FileError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("A custom error occurred")]
    OtherError,
}

pub trait MyResult {
    fn i(self) -> Self;
}

impl<T> MyResult for Result<T> {
    fn i(self) -> Self {
        match self {
            Err(y) => {
                eprintln!("Error: {}", y);
                Err(y)
            }
            x => x,
        }
    }
}

fn main() {
    let pdf_path = Path::new(
        "/mnt/c/Users/bradi/Downloads/new2.pdf",
    );
    let an_res = annotations::get_annotations(pdf_path).unwrap();

    let output = display_outline_item(&an_res, 1);

    // let to_append: Vec<String> = output.split("\n").map(String::from).collect();
    println!("{}", output)
}
