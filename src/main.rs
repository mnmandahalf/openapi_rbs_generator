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
                let rbs_definition = convert_schema_to_rbs(name, schema, spec);
                writeln!(file, "{}", rbs_definition)?;
            }
        }
    }
    for (path, path_item) in &spec.paths.paths {
        if let ReferenceOr::Item(path_item) = path_item {
            for (method, operation) in path_item.iter() {
                if let Some(request_body) = &operation.request_body {
                    match &request_body {
                        ReferenceOr::Item(request_body) => {
                            for (_content_type, media_type) in &request_body.content {
                                if let Some(schema) = &media_type.schema {
                                    if let ReferenceOr::Item(schema) = schema {
                                        let rbs_definition = convert_schema_to_rbs(
                                            &format!(
                                                "{}{}RequestBody",
                                                convert_path_to_camel_case(method),
                                                convert_path_to_camel_case(path)
                                            ),
                                            schema,
                                            spec,
                                        );
                                        writeln!(file, "{}", rbs_definition)?;
                                    }
                                }
                            }
                        }
                        // if request body is a reference, resolve it
                        ReferenceOr::Reference { reference } => {
                            resolve_reference_to_schema(reference, spec).map(|schema| {
                                let rbs_definition = convert_schema_to_rbs(
                                    &format!(
                                        "{}{}RequestBody",
                                        convert_path_to_camel_case(method),
                                        convert_path_to_camel_case(path)
                                    ),
                                    &schema,
                                    spec,
                                );
                                writeln!(file, "{}", rbs_definition).unwrap();
                            });
                        }
                    }
                }
                for (status_code, response) in &operation.responses.responses {
                    match &response {
                        ReferenceOr::Item(response) => {
                            for (_content_type, media_type) in &response.content {
                                if let Some(schema) = &media_type.schema {
                                    if let ReferenceOr::Item(schema) = schema {
                                        let rbs_definition = convert_schema_to_rbs(
                                            &format!(
                                                "{}{}Response",
                                                convert_path_to_camel_case(path),
                                                status_code
                                            ),
                                            schema,
                                            spec,
                                        );
                                        writeln!(file, "{}", rbs_definition)?;
                                    }
                                }
                            }
                        }
                        // if response is a reference, resolve it
                        ReferenceOr::Reference { reference } => {
                            resolve_reference_to_schema(reference, spec).map(|schema| {
                                let rbs_definition = convert_schema_to_rbs(
                                    &format!(
                                        "{}{}Response",
                                        convert_path_to_camel_case(path),
                                        status_code
                                    ),
                                    &schema,
                                    spec,
                                );
                                writeln!(file, "{}", rbs_definition).unwrap();
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn convert_path_to_camel_case(path: &str) -> String {
    path.split(|c| c == '/' || c == '_' || c == '{' || c == '}')
        .filter(|&s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().chain(chars).collect::<String>(),
            }
        })
        .collect::<Vec<String>>()
        .join("")
}

fn convert_schema_to_rbs(name: &str, schema: &Schema, spec: &OpenAPI) -> String {
    let mut rbs = format!("class {} < Struct\n", name);

    if let SchemaKind::Type(Type::Object(object_type)) = &schema.schema_kind {
        for (prop_name, prop_schema_ref) in &object_type.properties {
            if let ReferenceOr::Item(prop_schema) = prop_schema_ref {
                let prop_type = map_schema_type_to_rbs(prop_schema, spec);
                rbs.push_str(&format!("  {}: {}\n", prop_name, prop_type));
            }
        }
    }

    rbs.push_str("end\n");
    rbs
}

fn map_schema_type_to_rbs(schema: &Schema, spec: &OpenAPI) -> String {
    match &schema.schema_kind {
        SchemaKind::Type(Type::String(_)) => "String".to_string(),
        SchemaKind::Type(Type::Number(_)) => "Numeric".to_string(),
        SchemaKind::Type(Type::Integer(_)) => "Integer".to_string(),
        SchemaKind::Type(Type::Boolean(_)) => "Bool".to_string(),
        SchemaKind::Type(Type::Array(array_type)) => {
            if let Some(items) = &array_type.items {
                match &items {
                    ReferenceOr::Item(item_schema_box) => {
                        let item_type = map_schema_type_to_rbs(item_schema_box.as_ref(), spec);
                        format!("Array[{}]", item_type)
                    }
                    ReferenceOr::Reference { reference } => {
                        if let Some(schema) = resolve_reference_to_schema(reference, spec) {
                            let item_type = map_schema_type_to_rbs(&schema, spec);
                            format!("Array[{}]", item_type)
                        } else {
                            "Array[untyped]".to_string()
                        }
                    }
                }
            } else {
                "Array[untyped]".to_string()
            }
        }
        SchemaKind::Type(Type::Object(object_type)) => {
            let mut rbs = "{ ".to_string();
            let record_items = object_type
                .properties
                .iter()
                .map(|(prop_name, prop_schema_ref)| match prop_schema_ref {
                    ReferenceOr::Item(prop_schema) => {
                        let prop_key = match prop_name.as_str() {
                            "type" => ":'type'".to_string(),
                            "class" => ":'class'".to_string(),
                            _ => prop_name.to_string(),
                        };
                        let prop_type = map_schema_type_to_rbs(prop_schema, spec);
                        format!("{}: {}", prop_key, prop_type)
                    }
                    ReferenceOr::Reference { reference } => {
                        let prop_key = match prop_name.as_str() {
                            "type" => ":'type'".to_string(),
                            "class" => ":'class'".to_string(),
                            _ => prop_name.to_string(),
                        };
                        format!(
                            "{}: {}",
                            prop_key,
                            resolve_reference_to_schema_name(reference)
                        )
                    }
                })
                .collect::<Vec<String>>()
                .join(", ");
            rbs.push_str(&record_items);
            rbs.push_str(" }");
            rbs
        }
        _ => "untyped".to_string(),
    }
}

fn resolve_reference_to_schema_name(reference: &str) -> String {
    reference.split('/').last().unwrap().to_string()
}

fn resolve_reference_to_schema(reference: &str, spec: &OpenAPI) -> Option<Schema> {
    let schema_name = resolve_reference_to_schema_name(reference);
    if let Some(components) = &spec.components {
        if let Some(schema_ref) = components.schemas.get(&schema_name) {
            if let ReferenceOr::Item(schema) = schema_ref {
                return Some(schema.clone());
            }
        }
    }
    None
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
