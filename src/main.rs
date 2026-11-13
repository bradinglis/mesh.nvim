#![feature(try_trait_v2)]
#![feature(iter_intersperse)]

pub mod annotations;

use std::path::Path;

use crate::annotations::get_annotations;

fn main() {
    env_logger::init();
    let pdf_path = Path::new(
        "./tests/test_4.pdf",
    );
    let an_res = annotations::get_annotations(pdf_path).unwrap();

    // let to_append: Vec<String> = output.split("\n").map(String::from).collect();
    println!("{:#?}", an_res)
}
