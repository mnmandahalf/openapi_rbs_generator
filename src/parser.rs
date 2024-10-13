use openapiv3::{OpenAPI, ReferenceOr, Schema, SchemaKind, Type};
use serde_json::Result;
use std::fs::File;
use std::io::{self, Read, Write};

pub fn parse_json_to_openapi(file_path: &str) -> Result<OpenAPI> {
    let mut file = File::open(file_path).expect("file not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("something went wrong reading the file");

    let spec: OpenAPI = serde_json::from_str(&contents)?;

    Ok(spec)
}

pub fn generate_rbs_from_openapi(spec: &OpenAPI, output_path: &str) -> io::Result<()> {
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
                                    match schema {
                                        ReferenceOr::Item(schema) => {
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
                                        ReferenceOr::Reference { reference } => {
                                            resolve_reference_to_schema(reference, spec).map(
                                                |schema| {
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
                                                },
                                            );
                                        }
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
                                    match schema {
                                        ReferenceOr::Item(schema) => {
                                            let rbs_definition = convert_schema_to_rbs(
                                                &format!(
                                                    "{}{}{}Response",
                                                    convert_path_to_camel_case(method),
                                                    convert_path_to_camel_case(path),
                                                    status_code
                                                ),
                                                schema,
                                                spec,
                                            );
                                            writeln!(file, "{}", rbs_definition)?;
                                        }
                                        ReferenceOr::Reference { reference } => {
                                            let rbs_definition = convert_ref_directly_to_rbs(
                                                &format!(
                                                    "{}{}{}Response",
                                                    convert_path_to_camel_case(method),
                                                    convert_path_to_camel_case(path),
                                                    status_code
                                                ),
                                                &resolve_reference_to_schema_name(reference),
                                            );
                                            writeln!(file, "{}", rbs_definition).unwrap();
                                        }
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

#[test]
fn test_convert_path_to_camel_case() {
    assert_eq!(convert_path_to_camel_case("/pets"), "Pets");
    assert_eq!(convert_path_to_camel_case("/pets/{petId}"), "PetsPetId");
    assert_eq!(
        convert_path_to_camel_case("/pets/{petId}/photos"),
        "PetsPetIdPhotos"
    );
}

fn convert_schema_to_rbs(name: &str, schema: &Schema, spec: &OpenAPI) -> String {
    let mut rbs = format!("class {} < Struct\n", name);
    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(object_type)) => {
            for (prop_name, prop_schema_ref) in &object_type.properties {
                if let ReferenceOr::Item(prop_schema) = prop_schema_ref {
                    let prop_type = map_schema_type_to_rbs(prop_schema, spec);
                    rbs.push_str(&format!(
                        "  {}: {}\n",
                        escape_rbs_reserved_prop_name(prop_name),
                        prop_type
                    ));
                }
            }
        }
        SchemaKind::Type(Type::Array(array_type)) => {
            let attr_reader_name = name.to_lowercase();
            if let Some(items) = &array_type.items {
                match items {
                    ReferenceOr::Item(item_schema_box) => {
                        let item_type = map_schema_type_to_rbs(item_schema_box.as_ref(), spec);
                        rbs.push_str(&format!(
                            "  attr_reader :{}, type: Array[{}]\n",
                            attr_reader_name, item_type
                        ));
                    }
                    ReferenceOr::Reference { reference } => {
                        let item_type = resolve_reference_to_schema_name(reference);
                        rbs.push_str(&format!(
                            "  attr_reader :{}, type: Array[{}]\n",
                            attr_reader_name, item_type
                        ));
                    }
                }
            }
        }
        _ => {}
    }

    rbs.push_str("end\n");
    rbs
}

fn convert_ref_directly_to_rbs(name: &str, ref_name: &str) -> String {
    let rbs = format!("class {} < {}\nend\n", name, ref_name);
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
                        let prop_type = map_schema_type_to_rbs(prop_schema, spec);
                        format!(
                            "{}: {}",
                            escape_rbs_reserved_prop_name(prop_name),
                            prop_type
                        )
                    }
                    ReferenceOr::Reference { reference } => {
                        format!(
                            "{}: {}",
                            escape_rbs_reserved_prop_name(prop_name),
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
        SchemaKind::OneOf { one_of } => {
            let mut rbs = "{ ".to_string();
            let one_of_items = one_of
                .iter()
                .map(|schema_ref| match schema_ref {
                    ReferenceOr::Item(schema) => map_schema_type_to_rbs(schema, spec),
                    ReferenceOr::Reference { reference } => {
                        resolve_reference_to_schema_name(reference)
                    }
                })
                .collect::<Vec<String>>()
                .join(" | ");
            rbs.push_str(&one_of_items);
            rbs.push_str(" }");
            rbs
        }
        SchemaKind::AllOf { all_of } => all_of
            .iter()
            .map(|schema_ref| match schema_ref {
                ReferenceOr::Item(schema) => map_schema_type_to_rbs(schema, spec),
                ReferenceOr::Reference { reference } => resolve_reference_to_schema_name(reference),
            })
            .collect::<Vec<String>>()
            .join(" & "),
        SchemaKind::AnyOf { any_of } => {
            dbg!(any_of);
            "untyped".to_string()
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

fn escape_rbs_reserved_prop_name(name: &str) -> String {
    match name {
        "type" => ":'type'".to_string(),
        "class" => ":'class'".to_string(),
        _ => name.to_string(),
    }
}
