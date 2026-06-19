use serde::{Serialize, de};
use std::collections::HashMap;
use std::sync::Arc;

/// Dynamic value type for node inputs/outputs
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    /// Arc-backed variants for zero-copy sharing across fan-out edges
    StringArc(Arc<String>),
    BytesArc(Arc<Vec<u8>>),
    JsonArc(Arc<serde_json::Value>),
}

/// Tagged-format helper for backward-compatible deserialization
#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "value")]
enum ValueTagged {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    #[serde(rename = "Bytes")]
    Bytes(Vec<u8>),
    #[serde(rename = "Json")]
    Json(serde_json::Value),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    StringArc(Arc<String>),
    BytesArc(Arc<Vec<u8>>),
    JsonArc(Arc<serde_json::Value>),
}

impl From<ValueTagged> for Value {
    fn from(v: ValueTagged) -> Self {
        match v {
            ValueTagged::Null => Value::Null,
            ValueTagged::Bool(b) => Value::Bool(b),
            ValueTagged::Number(n) => Value::Number(n),
            ValueTagged::String(s) => Value::String(s),
            ValueTagged::Bytes(b) => Value::Bytes(b),
            ValueTagged::Json(j) => Value::Json(j),
            ValueTagged::Array(a) => Value::Array(a),
            ValueTagged::Object(o) => Value::Object(o),
            ValueTagged::StringArc(s) => Value::StringArc(s),
            ValueTagged::BytesArc(b) => Value::BytesArc(b),
            ValueTagged::JsonArc(j) => Value::JsonArc(j),
        }
    }
}

impl<'de> de::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use de::Error;
        /// Visitor that first tries tagged format, then falls back to type inference
        struct ValueVisitor;
        
        impl<'de> de::Visitor<'de> for ValueVisitor {
            type Value = Value;
            
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a Value (tagged or inferred)")
            }
            
            fn visit_bool<E: Error>(self, v: bool) -> Result<Value, E> {
                Ok(Value::Bool(v))
            }
            
            fn visit_i64<E: Error>(self, v: i64) -> Result<Value, E> {
                Ok(Value::Number(v as f64))
            }
            
            fn visit_u64<E: Error>(self, v: u64) -> Result<Value, E> {
                Ok(Value::Number(v as f64))
            }
            
            fn visit_f64<E: Error>(self, v: f64) -> Result<Value, E> {
                Ok(Value::Number(v))
            }
            
            fn visit_str<E: Error>(self, v: &str) -> Result<Value, E> {
                Ok(Value::String(v.to_owned()))
            }
            
            fn visit_string<E: Error>(self, v: String) -> Result<Value, E> {
                Ok(Value::String(v))
            }
            
            fn visit_none<E: Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            
            fn visit_unit<E: Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            
            fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut v = Vec::new();
                while let Some(item) = seq.next_element()? {
                    v.push(item);
                }
                Ok(Value::Array(v))
            }
            
            fn visit_map<M>(self, mut map: M) -> Result<Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                // Peek the first key to check if this is a tagged Value
                let mut entries: Vec<(String, serde_json::Value)> = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let val: serde_json::Value = map.next_value()?;
                    entries.push((key, val));
                }
                
                // Check if it looks like a tagged format (has a "type" key)
                let is_tagged = entries.iter().any(|(k, _)| k == "type")
                    && entries.len() == 2;
                
                if is_tagged {
                    // Reconstruct as tagged JSON and deserialize
                    let json_obj: serde_json::Map<String, serde_json::Value> = entries.into_iter().collect();
                    let tagged: ValueTagged = serde_json::from_value(serde_json::Value::Object(json_obj))
                        .map_err(|e| de::Error::custom(format!("tagged format error: {}", e)))?;
                    Ok(tagged.into())
                } else {
                    // Untagged object — collect as String-keyed Value map
                    let mut obj = HashMap::new();
                    for (key, json_val) in entries {
                        obj.insert(key, json_to_value(json_val));
                    }
                    Ok(Value::Object(obj))
                }
            }
        }
        
        deserializer.deserialize_any(ValueVisitor)
    }
}

/// Convert a serde_json::Value to our Value type (lossy — strings stay as String, not Bytes)
fn json_to_value(j: serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_f64() {
                Value::Number(i)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            Value::Object(obj.into_iter().map(|(k, v)| (k, json_to_value(v))).collect())
        }
    }
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            Value::StringArc(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Value::Json(j) => Some(j),
            Value::JsonArc(j) => Some(j),
            _ => None,
        }
    }

    /// Convert to a displayable string
    pub fn to_string(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            Value::StringArc(s) => (**s).clone(),
            Value::Bytes(b) => format!("<{} bytes>", b.len()),
            Value::BytesArc(b) => format!("<{} bytes>", b.len()),
            Value::Json(j) => j.to_string(),
            Value::JsonArc(j) => j.to_string(),
            Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Object(obj) => {
                let items: Vec<String> = obj.iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            Value::BytesArc(b) => Some(b),
            Value::String(s) => Some(s.as_bytes()),
            Value::StringArc(s) => Some(s.as_bytes()),
            _ => None,
        }
    }

    pub fn take_bytes(self) -> Option<Vec<u8>> {
        match self {
            Value::Bytes(b) => Some(b),
            Value::BytesArc(b) => match Arc::try_unwrap(b) {
                Ok(v) => Some(v),
                Err(arc) => Some((*arc).clone()),
            },
            Value::String(s) => Some(s.into_bytes()),
            Value::StringArc(s) => match Arc::try_unwrap(s) {
                Ok(v) => Some(v.into_bytes()),
                Err(arc) => Some((*arc).clone().into_bytes()),
            },
            _ => None,
        }
    }

    /// Convert owned variants to Arc-backed variants for cheap sharing
    pub fn to_arc(&self) -> Value {
        match self {
            Value::String(s) => Value::StringArc(Arc::new(s.clone())),
            Value::Bytes(b) => Value::BytesArc(Arc::new(b.clone())),
            Value::Json(j) => Value::JsonArc(Arc::new(j.clone())),
            Value::StringArc(_) | Value::BytesArc(_) | Value::JsonArc(_) => self.clone(),
            other => other.clone(),
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<serde_json::Value> for Value {
    fn from(j: serde_json::Value) -> Self {
        Value::Json(j)
    }
}

impl From<Vec<u8>> for Value {
    fn from(b: Vec<u8>) -> Self {
        Value::Bytes(b)
    }
}

impl From<&[u8]> for Value {
    fn from(b: &[u8]) -> Self {
        Value::Bytes(b.to_vec())
    }
}

impl From<HashMap<String, Value>> for Value {
    fn from(m: HashMap<String, Value>) -> Self {
        Value::Object(m)
    }
}
