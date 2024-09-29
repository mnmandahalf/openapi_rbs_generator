use clap::Parser;
use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use serde_json::Result;
use std::fs::File;
use std::io::{self, Read, Write};

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

fn parse_json_to_openapi(file_path: &str) -> Result<OpenAPI> {
    let mut file = File::open(file_path).expect("file not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("something went wrong reading the file");

    let spec: OpenAPI = serde_json::from_str(&contents)?;

    Ok(spec)
}

fn generate_rbs_from_openapi(spec: &OpenAPI, output_path: &str) -> io::Result<()> {
    let mut file = File::create(output_path)?;

    if let Some(components) = &spec.components {
        for (name, schema_ref) in &components.schemas {
            if let ReferenceOr::Item(schema) = schema_ref {
                let rbs_definition = convert_schema_to_rbs(name, schema);
                writeln!(file, "{}", rbs_definition)?;
            }
        }
    }
    Ok(())
}

fn convert_schema_to_rbs(name: &str, schema: &Schema) -> String {
    let mut rbs = format!("class {} < Struct\n", name);

    if let SchemaKind::Type(Type::Object(object_type)) = &schema.schema_kind {
        for (prop_name, prop_schema_ref) in &object_type.properties {
            if let ReferenceOr::Item(prop_schema) = prop_schema_ref {
                let prop_type = map_schema_type_to_rbs(prop_schema);
                rbs.push_str(&format!("  {}: {}\n", prop_name, prop_type));
            }
        }
    }

    rbs.push_str("end\n");
    rbs
}

fn map_schema_type_to_rbs(schema: &Schema) -> String {
    match &schema.schema_kind {
        SchemaKind::Type(Type::String(_)) => "String".to_string(),
        SchemaKind::Type(Type::Integer(_)) => "Integer".to_string(),
        SchemaKind::Type(Type::Boolean(_)) => "Bool".to_string(),
        SchemaKind::Type(Type::Array(array_type)) => {
            if let Some(items) = &array_type.items {
                match &items {
                    ReferenceOr::Item(item_schema_box) => {
                        let item_type = map_schema_type_to_rbs(item_schema_box.as_ref());
                        format!("Array[{}]", item_type)
                    }
                    ReferenceOr::Reference { .. } => "Array[untyped]".to_string(),
                }
            } else {
                "Array[untyped]".to_string()
            }
        }
        SchemaKind::Type(Type::Object(_)) => "Hash[untyped, untyped]".to_string(),
        _ => "untyped".to_string(),
    }
}

fn main() {
    let args = Args::parse();

    let input_path = &args.input;
    let output_path = &args.output;

    match parse_json_to_openapi(input_path) {
        Ok(spec) => match generate_rbs_from_openapi(&spec, output_path) {
            Ok(_) => println!("Successfully generated RBS file"),
            Err(e) => println!("Error writing RBS file: {}", e),
        },
        Err(e) => println!("Error parsing JSON: {}", e),
    }
}
