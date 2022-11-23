use anyhow::anyhow;
use serde_json::Value;

use crate::identifier;

/// returns the paths of of fields with type Identifier
/// please be aware that path uses '.' as separator. This means, if the property name contains '.', the
/// generated path would be invalid. But DataContract's meta-schema prevents from using names containing '.'
pub(super) fn get_identifiers_paths(json_schema: &Value) -> Result<Vec<String>, anyhow::Error> {
    let document_properties = json_schema
        .get("properties")
        .ok_or_else(|| anyhow!("the 'properties' property must exists in schema"))?;

    let mut identifiers_paths: Vec<String> = vec![];
    let mut to_visit: Vec<(&Value, String)> = vec![(document_properties, String::from(""))];

    while let Some((value, path)) = to_visit.pop() {
        match value {
            Value::Object(map) => {
                for (key_name, key_value) in map.iter() {
                    if key_value.is_object() || key_value.is_array() {
                        to_visit.push((key_value, create_new_path_to_object(&path, key_name)))
                    }
                }
                if is_identifier(map) {
                    identifiers_paths.push(path)
                }
            }
            // what about the definitions???
            Value::Array(arr) => {
                for (i, value) in arr.iter().enumerate() {
                    if value.is_object() {
                        to_visit.push((value, create_new_path_to_array(&path, i)))
                    }
                }
            }
            _ => {
                // ignore every other type
            }
        }
    }

    Ok(identifiers_paths)
}

fn create_new_path_to_array(current_path: &str, index: usize) -> String {
    format!("{}[{}]", current_path, index)
}

fn create_new_path_to_object(current_path: &str, key_name: &str) -> String {
    if current_path.is_empty() {
        return key_name.to_string();
    }
    format!("{}.{}", current_path, key_name)
}

fn is_identifier(map: &serde_json::Map<String, Value>) -> bool {
    if let Some(content_media_type) = map.get("contentMediaType") {
        return content_media_type == identifier::MEDIA_TYPE;
    }

    false
}

#[cfg(test)]
mod test {
    use serde_json::{json, Value};

    use crate::assert_error_contains;

    use super::get_identifiers_paths;

    #[test]
    fn returns_identifiers_paths() {
        let input_data = get_input_data();

        let result = get_identifiers_paths(&input_data);

        assert_eq!(
            [
                "arrayOfObjects.items[2].items[0].properties.withIdentifier",
                "arrayOfObjects.items[0].properties.withIdentifier",
                "arrayOfObject.items.properties.withIdentifier",
                "nestedObject.properties.withIdentifier",
                "withIdentifier",
            ]
            .to_vec(),
            result.expect("no errors should be returned")
        );
    }

    #[test]
    fn returns_error_because_of_invalid_document() {
        let input_data = json!({
            "data" : {
                "properties" :  {}
            }

        });

        let result = get_identifiers_paths(&input_data);

        assert_error_contains!(result, "the 'properties' property must exists in schema")
    }

    fn get_input_data() -> Value {
        json!({
            "properties": {
                "simple": {
                    "type": "string"
                },
                "withIdentifier": {
                    "contentMediaType" :"application/x.dash.dpp.identifier"
                },
                "nestedObject": {
                    "type": "object",
                    "properties": {
                        "simple": {
                            "type": "string"
                        },
                        "withIdentifier": {
                            "contentMediaType" :"application/x.dash.dpp.identifier"
                        }
                    }
                },
                "arrayOfObject": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "simple": {
                                "type": "string"
                            },
                            "withIdentifier": {
                                "contentMediaType" :"application/x.dash.dpp.identifier",
                            }
                        }
                    }
                },
                "arrayOfObjects": {
                    "type": "array",
                    "items": [
                        {
                            "type": "object",
                            "properties": {
                                "simple": {
                                    "type": "string"
                                },
                                "withIdentifier": {
                                    "contentMediaType" :"application/x.dash.dpp.identifier",
                                }
                            }
                        },
                        {
                            "type": "string"
                        },
                        {
                            "type": "array",
                            "items": [
                                {
                                    "type": "object",
                                    "properties": {
                                        "simple": {
                                            "type": "string"
                                        },
                                        "withIdentifier": {
                                            "type": "object",
                                            "contentMediaType" :"application/x.dash.dpp.identifier",
                                        }
                                    }
                                }
                            ]
                        }
                    ]
                }
            }
        })
    }
}
