mod json;
mod parser;

use std::{env, fs};

use fabparse::FabError;
use parser::parse_value;

fn main() {
    let src = fs::read_to_string(env::args().nth(1).expect("Expected file argument"))
        .expect("Failed to read file");
    let mut s = src.as_str();
    match parse_value::<FabError>(&mut s)
    {
        Ok(json) => {
            println!("{:#?}", json);
        }
        Err(err) => {
            eprintln!("{}", err);
            err.print_trace(s);
            std::process::exit(1);
        }
    }
}
