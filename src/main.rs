use clap::Parser;
mod parser;

#[derive(Parser, Debug)]
#[command(name = "OpenAPI to RBS Generator")]
#[command(author = "mnmandahalf")]
#[command(version = "0.1.0")]
#[command(about = "Generates RBS type definitions from OpenAPI schema")]

struct Args {
    // the input openapi json file path
    #[arg(short, long)]
    input: String,

    // the output rbs file path
    #[arg(short, long, default_value = "output.rbs")]
    output: String,
}

fn main() {
    let args = Args::parse();

    let input_path = &args.input;
    let output_path = &args.output;

    match parser::parse_json_to_openapi(input_path) {
        Ok(spec) => match parser::generate_rbs_from_openapi(&spec, output_path) {
            Ok(_) => println!("Successfully generated RBS file"),
            Err(e) => println!("Error writing RBS file: {}", e),
        },
        Err(e) => println!("Error parsing JSON: {}", e),
    }
}
