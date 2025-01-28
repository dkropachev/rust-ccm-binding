use std::collections::HashMap;
use serde_yaml::{Value};

/// Represents arbitrary data
#[derive(Debug, Clone)]
pub enum ScyllaConfig {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<ScyllaConfig>),
    Map(HashMap<String, ScyllaConfig>),
}

impl ScyllaConfig {
    pub fn to_yaml(&self) -> Value {
        match self {
            ScyllaConfig::Null => Value::Null,
            ScyllaConfig::Bool(b) => Value::Bool(*b),
            ScyllaConfig::Int(i) => Value::Number(serde_yaml::Number::from(*i)),
            ScyllaConfig::Float(f) => Value::Number(
                serde_yaml::Number::from(*f),
            ),
            ScyllaConfig::String(s) => Value::String(s.clone()),
            ScyllaConfig::List(list) => {
                let yaml_list: Vec<Value> = list.iter().map(|item| item.to_yaml()).collect();
                Value::Sequence(yaml_list)
            }
            ScyllaConfig::Map(map) => {
                let yaml_map: serde_yaml::Mapping = map
                    .iter()
                    .map(|(key, value)| (Value::String(key.clone()), value.to_yaml()))
                    .collect();
                Value::Mapping(yaml_map)
            }
        }
    }
    /// Parses a YAML string into a ClusterConfig structure
    pub fn from_yaml(value: Value) -> Result<ScyllaConfig, String> {
        match value {
            Value::Null => Ok(ScyllaConfig::Null),
            Value::Bool(b) => Ok(ScyllaConfig::Bool(b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ScyllaConfig::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(ScyllaConfig::Float(f))
                } else {
                    Err("Number is not an integer or float".to_string())
                }
            }
            Value::String(s) => Ok(ScyllaConfig::String(s)),
            Value::Sequence(seq) => {
                let mut new_seq = Vec::new();
                for value in seq {
                    if let Ok(parsed_value) = ScyllaConfig::from_yaml(value) {
                        new_seq.push(parsed_value);
                    } else {
                        return Err("Error parsing value in sequence".to_string());
                    }
                }
                Ok(ScyllaConfig::List(new_seq))
            }
            Value::Mapping(map) => {
                let mut new_map = HashMap::new();
                for (key, value) in map {
                    if let Value::String(key_str) = key {
                        if let Ok(parsed_value) = ScyllaConfig::from_yaml(value) {
                            new_map.insert(key_str, parsed_value);
                        } else {
                            return Err("Error parsing value in mapping".to_string());
                        }
                    } else {
                        return Err("Invalid key type in mapping".to_string());
                    }
                }
                Ok(ScyllaConfig::Map(new_map))
            }
            _ => Err("Unsupported YAML type".to_string()), // Explicitly handle unsupported types
        }
    }

    // Represents config in format 'l1key1.l2key1:val1 l1key1.l2key2:val2 l1key3:val3'
    pub fn to_flat_string(&self) -> String {
        fn flatten_map(
            map: &HashMap<String, ScyllaConfig>,
            prefix: String,
            output: &mut Vec<String>,
        ) {
            // Sort keys before processing
            let mut sorted_keys: Vec<&String> = map.keys().collect();
            sorted_keys.sort();

            for key in sorted_keys {
                let value = map.get(key).unwrap();
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                match value {
                    ScyllaConfig::Map(inner_map) => {
                        flatten_map(inner_map, full_key, output);
                    }
                    ScyllaConfig::String(s) => {
                        output.push(format!("{}:{}", full_key, s));
                    }
                    ScyllaConfig::Int(i) => {
                        output.push(format!("{}:{}", full_key, i));
                    }
                    ScyllaConfig::Float(f) => {
                        output.push(format!("{}:{}", full_key, f));
                    }
                    ScyllaConfig::Bool(b) => {
                        output.push(format!("{}:{}", full_key, b));
                    }
                    ScyllaConfig::Null => {
                        output.push(format!("{}:null", full_key));
                    }
                    ScyllaConfig::List(list) => {
                        let list_str = list
                            .iter()
                            .map(|item| format!("{:?}", item))
                            .collect::<Vec<_>>()
                            .join(", ");
                        output.push(format!("{}:[{}]", full_key, list_str));
                    }
                }
            }
        }

        let mut result = Vec::new();
        if let ScyllaConfig::Map(map) = self {
            flatten_map(map, String::new(), &mut result);
        }
        result.join(" ")
    }

    /// Returns a mutable reference to the output of the future.
    /// The output of this method will be [`Some`] if and only if the inner
    /// future has been completed and [`take_output`](MaybeDone::take_output)
    /// has not yet been called.
    pub fn output_mut(self: &mut ScyllaConfig) -> Option<&mut ScyllaConfig> {
        match self {
            ScyllaConfig::List(list) => list.last_mut(),
            ScyllaConfig::Map(map) => map.values_mut().last(),
            _ => None,
        }
    }

    /// Attempt to take the output of a `MaybeDone` without driving it
    /// towards completion.
    pub fn take_output(self: &mut ScyllaConfig) -> Option<ScyllaConfig> {
        match self {
            ScyllaConfig::List(list) => list.pop(),
            ScyllaConfig::Map(map) => map.values_mut().next().map(|value| value.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    #[test]
    fn test_from_yaml_and_to_yaml() {
        // Define a sample YAML string
        let yaml_str = r#"
            null_value: null
            bool_value: true
            int_value: 42
            float_value: 3.14
            string_value: "hello"
            list_value:
              - 1
              - 2
              - 3
            map_value:
              key1: "value1"
              key2: 99
        "#;

        // Parse the YAML string into a serde_yaml::Value
        let yaml_value: Value = serde_yaml::from_str(yaml_str).expect("Failed to parse YAML");

        // Convert from YAML to ClusterConfig
        let cluster_config = ScyllaConfig::from_yaml(yaml_value.clone())
            .expect("Failed to convert from YAML to ClusterConfig");

        // Convert back from ClusterConfig to YAML
        let converted_yaml_value = cluster_config.to_yaml();

        // Assert the conversion is accurate by comparing original and converted YAML values
        assert_eq!(yaml_value, converted_yaml_value);
    }

    #[test]
    fn test_to_yaml_empty_structures() {
        // Test empty list
        let empty_list = ScyllaConfig::List(vec![]);
        assert_eq!(empty_list.to_yaml(), Value::Sequence(vec![]));

        // Test empty map
        let empty_map = ScyllaConfig::Map(HashMap::new());
        assert_eq!(empty_map.to_yaml(), Value::Mapping(serde_yaml::Mapping::new()));
    }

    #[test]
    fn test_from_yaml_invalid_cases() {
        // Test unsupported YAML type (e.g., unhashable keys)
        let invalid_yaml = Value::Mapping({
            let mut map = serde_yaml::Mapping::new();
            map.insert(Value::Sequence(vec![]), Value::Null);
            map
        });

        let result = ScyllaConfig::from_yaml(invalid_yaml);
        assert!(result.is_err(), "Expected error for invalid YAML type");
    }

    #[test]
    fn test_to_flat_string_simple_map() {
        let mut map = HashMap::new();
        map.insert("key1".to_string(), ScyllaConfig::String("value1".to_string()));
        map.insert("key2".to_string(), ScyllaConfig::Int(42));

        let cluster_config = ScyllaConfig::Map(map);
        let flat_representation = cluster_config.to_flat_string();

        assert_eq!(flat_representation, "key1:value1 key2:42");
    }

    #[test]
    fn test_to_flat_string_nested_map() {
        let mut inner_map = HashMap::new();
        inner_map.insert("inner_key".to_string(), ScyllaConfig::Bool(true));

        let mut outer_map = HashMap::new();
        outer_map.insert("outer_key1".to_string(), ScyllaConfig::Map(inner_map));
        outer_map.insert("outer_key2".to_string(), ScyllaConfig::Float(3.14));

        let cluster_config = ScyllaConfig::Map(outer_map);
        let flat_representation = cluster_config.to_flat_string();

        assert_eq!(
            flat_representation,
            "outer_key1.inner_key:true outer_key2:3.14"
        );
    }

    #[test]
    fn test_to_flat_string_with_empty_map() {
        let empty_map = HashMap::new();
        let cluster_config = ScyllaConfig::Map(empty_map);
        let flat_representation = cluster_config.to_flat_string();

        assert_eq!(flat_representation, "");
    }

    #[test]
    fn test_to_flat_string_with_list() {
        let list = vec![
            ScyllaConfig::Int(1),
            ScyllaConfig::Int(2),
            ScyllaConfig::String("three".to_string()),
        ];

        let mut map = HashMap::new();
        map.insert("key_with_list".to_string(), ScyllaConfig::List(list));

        let cluster_config = ScyllaConfig::Map(map);
        let flat_representation = cluster_config.to_flat_string();

        // Lists are serialized as comma-separated values in brackets.
        assert_eq!(
            flat_representation,
            "key_with_list:[Int(1), Int(2), String(\"three\")]"
        );
    }

    #[test]
    fn test_to_flat_string_with_null() {
        let mut map = HashMap::new();
        map.insert("null_key".to_string(), ScyllaConfig::Null);

        let cluster_config = ScyllaConfig::Map(map);
        let flat_representation = cluster_config.to_flat_string();

        assert_eq!(flat_representation, "null_key:null");
    }
}
