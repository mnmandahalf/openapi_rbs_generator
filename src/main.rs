use clap::{App, Arg};
use std::fs::File;
use std::io::Read;
use openapiv3::OpenAPI;
use serde_json::Result;

fn read_json_file(file_path: &str) -> Result<OpenAPI> {
    let mut file = File::open(file_path).expect("file not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("something went wrong reading the file");

    let spec: OpenAPI = serde_json::from_str(&contents)?;

    Ok(spec)
}

fn main() {
    let matches = App::new("openapi-parser")
        .version("1.0")
        .author("mnmandahalf")
        .about("Parses OpenAPI JSON files")
        .arg(Arg::with_name("FILE")
             .help("Sets the input file to use")
             .required(true)
             .index(1))
        .get_matches();

    let file_path = matches.value_of("FILE").unwrap();
    match read_json_file(file_path) {
        Ok(data) => println!("{:?}", data),
        Err(e) => println!("Error parsing JSON: {}", e),
    }
}
