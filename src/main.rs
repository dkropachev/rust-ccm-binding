// mod ClusterConfig;
//
// use std::collections::HashMap;
//
// #[derive(Debug, Clone)]
// pub enum DataRequirement {
//     Any,                     // Requires value to be null
//     Null,                     // Requires value to be null
//     Bool(bool),               // Requires value to match the specified boolean
//     Int { min: Option<i64>, max: Option<i64> }, // Range constraints for integers
//     IntIn(Option<Vec<i64>>), // In constraints for integers
//     Float { min: Option<f64>, max: Option<f64> }, // Range constraints for floats
//     FloatIn(Option<Vec<f64>>), // In constraints for floats
//     String { contains: Option<String>, regex: Option<String> }, // String constraints
//     StringIn(Option<Vec<String>>), // String constraints
//     List(Vec<DataRequirement>), // Constraints for list elements
//     ListIn(Vec<DataRequirement>), // Constraints for list elements
//     Map(HashMap<String, Box<DataRequirement>>), // Constraints for map keys and values
//     MapIn(Vec<HashMap<String, Box<DataRequirement>>>), // In Constraints for map keys and values
//     And(Vec<DataRequirement>), // Logical AND of multiple requirements
//     Or(Vec<DataRequirement>), // Logical OR of multiple requirements
//     Not(Box<DataRequirement>), // Logical NOT of a requirement
// }
//
// impl DataRequirement {
//     /// Validate a given `DataValue` against the `DataRequirement`.
//     pub fn validate(&self, value: &DataValue) -> bool {
//         match (self, value) {
//             (DataRequirement::Any, _) => true,
//             (DataRequirement::Null, DataValue::Null) => true,
//             (DataRequirement::Bool(expected), DataValue::Bool(actual)) => expected == actual,
//             (DataRequirement::Int { min, max }, DataValue::Int(actual)) => {
//                 min.map_or(true, |m| *actual >= m) && max.map_or(true, |m| *actual <= m)
//             }
//             (DataRequirement::IntIn(allowed), DataValue::Int(actual)) => {
//                 allowed.as_ref().map_or(true, |allowed| allowed.contains(actual))
//             }
//             (DataRequirement::Float { min, max }, DataValue::Float(actual)) => {
//                 min.map_or(true, |m| *actual >= m) && max.map_or(true, |m| *actual <= m)
//             }
//             (DataRequirement::FloatIn(allowed), DataValue::Float(actual)) => {
//                 allowed.as_ref().map_or(true, |allowed| allowed.contains(actual))
//             }
//             (DataRequirement::String { contains, regex }, DataValue::String(actual)) => {
//                 let contains_match = contains.as_ref().map_or(true, |c| actual.contains(c));
//                 let regex_match = regex
//                     .as_ref()
//                     .map_or(true, |r| regex::Regex::new(r).map_or(false, |re| re.is_match(actual)));
//                 contains_match && regex_match
//             }
//             (DataRequirement::StringIn(allowed), DataValue::String(actual)) => {
//                 allowed.as_ref().map_or(true, |allowed| allowed.contains(actual))
//             }
//             (DataRequirement::List(requirements), DataValue::List(values)) => {
//                 requirements.iter().zip(values).all(|(req, val)| req.validate(val))
//             }
//             (DataRequirement::ListIn(allowed), DataValue::List(values)) => {
//                 allowed.iter().zip(values).any(|(req, val)| req.validate(val))
//             }
//             (DataRequirement::Map(requirements), DataValue::Map(values)) => {
//                 requirements.iter().all(|(key, req)| {
//                     values.get(key).map_or(false, |val| req.validate(val))
//                 })
//             }
//             (DataRequirement::MapIn(allowed), DataValue::Map(values)) => {
//                 allowed.iter().any(|rec| rec.iter().all(|(key, req)| {
//                     values.get(key).map_or(false, |val| req.validate(val))
//                 }))
//             }
//             (DataRequirement::And(requirements), value) => {
//                 requirements.iter().all(|req| req.validate(value))
//             }
//             (DataRequirement::Or(requirements), value) => {
//                 requirements.iter().any(|req| req.validate(value))
//             }
//             (DataRequirement::Not(requirement), value) => {
//                 !requirement.validate(value)
//             }
//             _ => false,
//         }
//     }
//
//     /// Generate a `DataValue` that satisfies all the provided `DataRequirement`s.
//     pub fn generate_matching_value(requirements: Vec<DataRequirement>) -> Option<DataValue> {
//         use std::cmp::Ordering;
//
//         if requirements.is_empty() {
//             return None;
//         }
//
//         let mut result = None;
//
//         for req in requirements {
//             match req {
//                 DataRequirement::Null => result = Some(DataValue::Null),
//                 DataRequirement::Bool(expected) => result = Some(DataValue::Bool(expected)),
//                 DataRequirement::Int { min, max } => {
//                     let value = min.unwrap_or(i64::MIN);
//                     if max.map_or(true, |m| value <= m) {
//                         result = Some(DataValue::Int(value));
//                     } else {
//                         return None; // No valid value within range
//                     }
//                 }
//                 DataRequirement::IntIn(Some(allowed)) => {
//                     if let Some(&value) = allowed.iter().min() {
//                         result = Some(DataValue::Int(value));
//                     } else {
//                         return None;
//                     }
//                 }
//                 DataRequirement::Float { min, max } => {
//                     let value = min.unwrap_or(f64::MIN);
//                     if max.map_or(true, |m| value <= m) {
//                         result = Some(DataValue::Float(value));
//                     } else {
//                         return None; // No valid value within range
//                     }
//                 }
//                 DataRequirement::FloatIn(Some(allowed)) => {
//                     if let Some(&value) = allowed.iter().min_by(|a, b| {
//                         if a < b {
//                             Ordering::Less
//                         } else if a > b {
//                             Ordering::Greater
//                         } else {
//                             Ordering::Equal
//                         }
//                     }) {
//                         result = Some(DataValue::Float(value));
//                     } else {
//                         return None;
//                     }
//                 }
//                 DataRequirement::String { contains, regex } => {
//                     if let Some(c) = contains {
//                         result = Some(DataValue::String(c));
//                     } else if regex.is_some() {
//                         result = Some(DataValue::String(String::from("matching")));
//                     } else {
//                         return None;
//                     }
//                 }
//                 DataRequirement::StringIn(Some(allowed)) => {
//                     if let Some(value) = allowed.into_iter().min() {
//                         result = Some(DataValue::String(value));
//                     } else {
//                         return None;
//                     }
//                 }
//                 DataRequirement::List(reqs) => {
//                     let mut values = Vec::new();
//                     for req in reqs.into_iter() {
//                         if let Some(value) = DataRequirement::generate_matching_value(vec![req]) {
//                             values.push(value);
//                         } else {
//                             return None;
//                         }
//                     }
//                     result = Some(DataValue::List(values));
//                 }
//                 // DataRequirement::List(reqs) => {
//                 //     let mut values = Vec::new();
//                 //     for req in reqs {
//                 //         if let Some(value) = DataRequirement::generate_matching_value(vec![req]) {
//                 //             values.push(value);
//                 //         } else {
//                 //             return None;
//                 //         }
//                 //     }
//                 //     result = Some(DataValue::List(values));
//                 // }
//                 DataRequirement::ListIn(allowed) => {
//                     if let Some(reqs) = allowed.first() {
//                         let mut values = Vec::new();
//                         for req in reqs.into_iter() {
//                             if let Some(value) = DataRequirement::generate_matching_value(vec![req.clone()]) {
//                                 values.push(value);
//                             } else {
//                                 return None;
//                             }
//                         }
//                         result = Some(DataValue::List(values));
//                     }
//                 }
//                 DataRequirement::Map(req_map) => {
//                     let mut map = HashMap::new();
//                     for (key, req) in req_map {
//                         if let Some(value) = DataRequirement::generate_matching_value(vec![*req]) {
//                             map.insert(key, value);
//                         } else {
//                             return None;
//                         }
//                     }
//                     result = Some(DataValue::Map(map));
//                 }
//                 DataRequirement::MapIn(allowed) => {
//                     if let Some(req_map) = allowed.first() {
//                         let mut map = HashMap::new();
//                         for (key, req) in req_map {
//                             if let Some(value) =
//                                 DataRequirement::generate_matching_value(vec![*req.clone()])
//                             {
//                                 map.insert(key.clone(), value);
//                             } else {
//                                 return None;
//                             }
//                         }
//                         result = Some(DataValue::Map(map));
//                     }
//                 }
//                 DataRequirement::And(reqs) => {
//                     let mut intermediate = vec![];
//                     for req in reqs {
//                         intermediate.push(req);
//                     }
//                     result = DataRequirement::generate_matching_value(intermediate);
//                 }
//                 DataRequirement::Or(reqs) => {
//                     for req in reqs {
//                         if let Some(value) = DataRequirement::generate_matching_value(vec![req]) {
//                             return Some(value);
//                         }
//                     }
//                     return None;
//                 }
//                 DataRequirement::Not(_) => return None, // Cannot satisfy NOT logically
//                 _ => {}
//             }
//         }
//
//         result
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::collections::HashMap;
//
//     #[test]
//     fn test_data_requirement_null() {
//         assert!(DataRequirement::Null.validate(&DataValue::Null));
//         assert!(!DataRequirement::Null.validate(&DataValue::Int(5)));
//     }
//
//     #[test]
//     fn test_data_requirement_bool() {
//         assert!(DataRequirement::Bool(true).validate(&DataValue::Bool(true)));
//         assert!(!DataRequirement::Bool(true).validate(&DataValue::Bool(false)));
//     }
//
//     #[test]
//     fn test_data_requirement_int() {
//         let req = DataRequirement::Int { min: Some(5), max: Some(10) };
//         assert!(req.validate(&DataValue::Int(7)));
//         assert!(!req.validate(&DataValue::Int(4)));
//         assert!(!req.validate(&DataValue::Int(11)));
//
//         let req = DataRequirement::Int { min: None, max: Some(10) };
//         assert!(req.validate(&DataValue::Int(10)));
//         assert!(!req.validate(&DataValue::Int(11)));
//
//         let req = DataRequirement::Int { min: Some(5), max: None };
//         assert!(req.validate(&DataValue::Int(6)));
//         assert!(!req.validate(&DataValue::Int(4)));
//     }
//
//     #[test]
//     fn test_data_requirement_int_in() {
//         let req = DataRequirement::IntIn(Some(vec![1, 2, 3]));
//         assert!(req.validate(&DataValue::Int(2)));
//         assert!(!req.validate(&DataValue::Int(4)));
//     }
//
//     #[test]
//     fn test_data_requirement_float() {
//         let req = DataRequirement::Float { min: Some(1.5), max: Some(3.5) };
//         assert!(req.validate(&DataValue::Float(2.5)));
//         assert!(!req.validate(&DataValue::Float(4.0)));
//     }
//
//     #[test]
//     fn test_data_requirement_float_in() {
//         let req = DataRequirement::FloatIn(Some(vec![1.1, 2.2, 3.3]));
//         assert!(req.validate(&DataValue::Float(2.2)));
//         assert!(!req.validate(&DataValue::Float(4.4)));
//     }
//
//     #[test]
//     fn test_data_requirement_string() {
//         let req = DataRequirement::String {
//             contains: Some("test".to_string()),
//             regex: Some("^test.*$".to_string()),
//         };
//         assert!(req.validate(&DataValue::String("test123".to_string())));
//         assert!(!req.validate(&DataValue::String("123".to_string())));
//     }
//
//     #[test]
//     fn test_data_requirement_string_in() {
//         let req = DataRequirement::StringIn(Some(vec!["one".to_string(), "two".to_string()]));
//         assert!(req.validate(&DataValue::String("one".to_string())));
//         assert!(!req.validate(&DataValue::String("three".to_string())));
//     }
//
//     #[test]
//     fn test_data_requirement_list() {
//         let req = DataRequirement::List(vec![
//             DataRequirement::Int { min: Some(1), max: Some(10) },
//             DataRequirement::Bool(true),
//         ]);
//         assert!(req.validate(&DataValue::List(vec![
//             DataValue::Int(5),
//             DataValue::Bool(true),
//         ])));
//         assert!(!req.validate(&DataValue::List(vec![
//             DataValue::Int(11),
//             DataValue::Bool(false),
//         ])));
//     }
//
//     #[test]
//     fn test_data_requirement_list_in() {
//         let req = DataRequirement::ListIn(vec![
//             DataRequirement::Int { min: Some(1), max: Some(10) },
//             DataRequirement::Bool(true),
//         ]);
//         assert!(req.validate(&DataValue::List(vec![
//             DataValue::Int(5),
//             DataValue::Bool(true),
//         ])));
//     }
//
//     #[test]
//     fn test_data_requirement_map() {
//         let mut map_req = HashMap::new();
//         map_req.insert("key1".to_string(), Box::new(DataRequirement::Int { min: Some(1), max: Some(5) }));
//         map_req.insert("key2".to_string(), Box::new(DataRequirement::Bool(true)));
//
//         let mut map_val = HashMap::new();
//         map_val.insert("key1".to_string(), DataValue::Int(3));
//         map_val.insert("key2".to_string(), DataValue::Bool(true));
//
//         let req = DataRequirement::Map(map_req);
//         assert!(req.validate(&DataValue::Map(map_val.clone())));
//
//         map_val.insert("key1".to_string(), DataValue::Int(6));
//         assert!(!req.validate(&DataValue::Map(map_val)));
//     }
//
//     #[test]
//     fn test_data_requirement_and_or_not() {
//         let req = DataRequirement::And(vec![
//             DataRequirement::List(vec![
//                 DataRequirement::Int { min: Some(1), max: Some(5) },
//                 DataRequirement::Bool(true),
//             ]),
//         ]);
//         assert!(req.validate(&DataValue::List(vec![
//             DataValue::Int(3),
//             DataValue::Bool(true),
//         ])));
//
//         let req = DataRequirement::Or(vec![
//             DataRequirement::Int { min: Some(1), max: Some(5) },
//             DataRequirement::Bool(false),
//         ]);
//         assert!(req.validate(&DataValue::Int(3)));
//
//         let req = DataRequirement::Not(Box::new(DataRequirement::Bool(false)));
//         assert!(req.validate(&DataValue::Bool(true)));
//     }
//
//
//     #[test]
//     fn test_generate_matching_value_null() {
//         let requirements = vec![DataRequirement::Null];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Null));
//     }
//
//     #[test]
//     fn test_generate_matching_value_bool() {
//         let requirements = vec![DataRequirement::Bool(true)];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Bool(true)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_int_min() {
//         let requirements = vec![DataRequirement::Int {
//             min: Some(10),
//             max: Some(20),
//         }];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Int(10)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_int_in() {
//         let requirements = vec![DataRequirement::IntIn(Some(vec![5, 10, 15]))];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Int(5)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_float_min() {
//         let requirements = vec![DataRequirement::Float {
//             min: Some(1.5),
//             max: Some(3.5),
//         }];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Float(1.5)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_float_in() {
//         let requirements = vec![DataRequirement::FloatIn(Some(vec![2.5, 3.5, 4.5]))];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Float(2.5)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_string_contains() {
//         let requirements = vec![DataRequirement::String {
//             contains: Some("test".to_string()),
//             regex: None,
//         }];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::String("test".to_string())));
//     }
//
//     #[test]
//     fn test_generate_matching_value_string_in() {
//         let requirements = vec![DataRequirement::StringIn(Some(vec![
//             "alpha".to_string(),
//             "beta".to_string(),
//         ]))];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::String("alpha".to_string())));
//     }
//
//     #[test]
//     fn test_generate_matching_value_list() {
//         let requirements = vec![DataRequirement::List(vec![
//             DataRequirement::Int {
//                 min: Some(1),
//                 max: Some(5),
//             },
//             DataRequirement::Bool(true),
//         ])];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(
//             result,
//             Some(DataValue::List(vec![
//                 DataValue::Int(1),
//                 DataValue::Bool(true)
//             ]))
//         );
//     }
//
//     #[test]
//     fn test_generate_matching_value_map() {
//         let mut map_reqs = HashMap::new();
//         map_reqs.insert(
//             "key1".to_string(),
//             Box::new(DataRequirement::Int {
//                 min: Some(10),
//                 max: Some(20),
//             }),
//         );
//         map_reqs.insert(
//             "key2".to_string(),
//             Box::new(DataRequirement::Bool(false)),
//         );
//         let requirements = vec![DataRequirement::Map(map_reqs)];
//         let result = DataRequirement::generate_matching_value(requirements);
//
//         let mut expected_map = HashMap::new();
//         expected_map.insert("key1".to_string(), DataValue::Int(10));
//         expected_map.insert("key2".to_string(), DataValue::Bool(false));
//
//         assert_eq!(result, Some(DataValue::Map(expected_map)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_and() {
//         let requirements = vec![DataRequirement::And(vec![
//             DataRequirement::Int {
//                 min: Some(5),
//                 max: Some(15),
//             },
//             DataRequirement::Int {
//                 min: Some(10),
//                 max: Some(20),
//             },
//         ])];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Int(10)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_or() {
//         let requirements = vec![DataRequirement::Or(vec![
//             DataRequirement::Int {
//                 min: Some(10),
//                 max: Some(20),
//             },
//             DataRequirement::Int {
//                 min: Some(5),
//                 max: Some(15),
//             },
//         ])];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, Some(DataValue::Int(10)));
//     }
//
//     #[test]
//     fn test_generate_matching_value_not() {
//         let requirements = vec![DataRequirement::Not(Box::new(DataRequirement::Bool(true)))];
//         let result = DataRequirement::generate_matching_value(requirements);
//         assert_eq!(result, None); // Not constraints cannot logically produce a value
//     }
// }
//
//
// fn main() {
//     let yaml_data = r#"
//     key1: value1
//     key2: 42
//     key3:
//       nested_key: nested_value
//     key4:
//       - item1
//       - 2
//       - false
//     "#;
//
//     match parse_to_data_value(yaml_data) {
//         Ok(data) => println!("Parsed Data: {:#?}", data),
//         Err(err) => eprintln!("Error: {}", err),
//     }
//
//     let mut requirements = HashMap::new();
//     requirements.insert(
//         "intKey".to_string(),
//         Box::new(DataRequirement::Int {
//             min: Some(10),
//             max: None,
//         }),
//     );
//     requirements.insert(
//         "stringKey".to_string(),
//         Box::new(DataRequirement::String {
//             contains: Some("str".to_string()),
//             regex: None,
//         }),
//     );
//
//     let requirement = DataRequirement::Map(requirements);
//
//     let mut values = HashMap::new();
//     values.insert("intKey".to_string(), DataValue::Int(30));
//     values.insert("stringKey".to_string(), DataValue::String("strVal".to_string()));
//
//     let data = DataValue::Map(values);
//
//     let is_valid = requirement.validate(&data);
//     println!("Validation result: {}", is_valid); // Output: Validation result: true
// }

mod cluster_config;
mod find_available_iprange;
mod cluster;

fn main() {

}
