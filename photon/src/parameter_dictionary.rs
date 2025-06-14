use std::collections::HashMap;
use std::collections::hash_map::{Iter, IntoIter};
use std::ops::{Index, IndexMut};

/// An enum representing the different types of values that can be stored in a ParameterDictionary
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Boolean(bool),
    Byte(u8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Null,
    ByteArray(Vec<u8>),
    StringArray(Vec<String>),
    // Add more types as needed based on the GpType enum
}

/// A dictionary that maps byte keys to values of various types
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDictionary {
    param_dict: HashMap<u8, Value>,
}

impl ParameterDictionary {
    /// Creates a new, empty ParameterDictionary
    pub fn new() -> Self {
        ParameterDictionary {
            param_dict: HashMap::new(),
        }
    }

    /// Creates a new ParameterDictionary with the specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        ParameterDictionary {
            param_dict: HashMap::with_capacity(capacity),
        }
    }

    /// Gets the value associated with the specified key
    pub fn get(&self, key: u8) -> Option<&Value> {
        self.param_dict.get(&key)
    }

    /// Sets the value associated with the specified key
    pub fn set(&mut self, key: u8, value: Value) {
        self.param_dict.insert(key, value);
    }

    /// Returns the number of key-value pairs in the dictionary
    pub fn count(&self) -> usize {
        self.param_dict.len()
    }

    /// Returns an iterator over the key-value pairs in the dictionary
    pub fn iter(&self) -> Iter<'_, u8, Value> {
        self.param_dict.iter()
    }

    /// Checks if the dictionary contains the specified key
    pub fn contains_key(&self, key: u8) -> bool {
        self.param_dict.contains_key(&key)
    }

    /// Removes the value associated with the specified key
    pub fn remove(&mut self, key: u8) -> Option<Value> {
        self.param_dict.remove(&key)
    }

    /// Clears all key-value pairs from the dictionary
    pub fn clear(&mut self) {
        self.param_dict.clear();
    }
}

impl Index<u8> for ParameterDictionary {
    type Output = Value;

    fn index(&self, key: u8) -> &Self::Output {
        self.param_dict.get(&key).expect("No value found for key")
    }
}

impl IndexMut<u8> for ParameterDictionary {
    fn index_mut(&mut self, key: u8) -> &mut Self::Output {
        self.param_dict.entry(key).or_insert(Value::Null)
    }
}

impl IntoIterator for ParameterDictionary {
    type Item = (u8, Value);
    type IntoIter = IntoIter<u8, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.param_dict.into_iter()
    }
}

impl<'a> IntoIterator for &'a ParameterDictionary {
    type Item = (&'a u8, &'a Value);
    type IntoIter = Iter<'a, u8, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.param_dict.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let dict = ParameterDictionary::new();
        assert_eq!(dict.count(), 0);
    }

    #[test]
    fn test_with_capacity() {
        let dict = ParameterDictionary::with_capacity(10);
        assert_eq!(dict.count(), 0);
    }

    #[test]
    fn test_set_get() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));
        dict.set(2, Value::Int(42));
        dict.set(3, Value::String("hello".to_string()));

        match dict.get(1) {
            Some(Value::Boolean(val)) => assert_eq!(*val, true),
            _ => panic!("Expected Boolean(true)"),
        }

        match dict.get(2) {
            Some(Value::Int(val)) => assert_eq!(*val, 42),
            _ => panic!("Expected Int(42)"),
        }

        match dict.get(3) {
            Some(Value::String(val)) => assert_eq!(val, "hello"),
            _ => panic!("Expected String(\"hello\")"),
        }

        assert_eq!(dict.get(4), None);
    }

    #[test]
    fn test_indexing() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));

        match dict[1] {
            Value::Boolean(val) => assert_eq!(val, true),
            _ => panic!("Expected Boolean(true)"),
        }

        dict[2] = Value::Int(42);
        match dict[2] {
            Value::Int(val) => assert_eq!(val, 42),
            _ => panic!("Expected Int(42)"),
        }
    }

    #[test]
    #[should_panic(expected = "No value found for key")]
    fn test_index_panic() {
        let dict = ParameterDictionary::new();
        let _ = dict[1]; // This should panic
    }

    #[test]
    fn test_contains_key() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));

        assert!(dict.contains_key(1));
        assert!(!dict.contains_key(2));
    }

    #[test]
    fn test_remove() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));

        let removed = dict.remove(1);
        match removed {
            Some(Value::Boolean(val)) => assert_eq!(val, true),
            _ => panic!("Expected Boolean(true)"),
        }

        assert_eq!(dict.count(), 0);
        assert!(!dict.contains_key(1));
    }

    #[test]
    fn test_clear() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));
        dict.set(2, Value::Int(42));

        dict.clear();
        assert_eq!(dict.count(), 0);
        assert!(!dict.contains_key(1));
        assert!(!dict.contains_key(2));
    }

    #[test]
    fn test_iteration() {
        let mut dict = ParameterDictionary::new();
        dict.set(1, Value::Boolean(true));
        dict.set(2, Value::Int(42));

        let mut count = 0;
        for (key, value) in &dict {
            count += 1;
            match *key {
                1 => match value {
                    Value::Boolean(val) => assert_eq!(*val, true),
                    _ => panic!("Expected Boolean(true)"),
                },
                2 => match value {
                    Value::Int(val) => assert_eq!(*val, 42),
                    _ => panic!("Expected Int(42)"),
                },
                _ => panic!("Unexpected key"),
            }
        }

        assert_eq!(count, 2);
    }
}
