use plist::{Dictionary, Uid, Value};
use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use super::types::APPLE_EPOCH_UNIX_SECONDS;
use crate::error::{Result, SnipError};

#[derive(Clone, Debug)]
pub(crate) enum Decoded {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Real(f64),
    String(String),
    Data(Vec<u8>),
    Date(String),
    Array(Vec<Decoded>),
    Dict(BTreeMap<String, Decoded>),
}

impl Decoded {
    pub(crate) fn as_dict(&self) -> Option<&BTreeMap<String, Decoded>> {
        match self {
            Self::Dict(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn as_array(&self) -> Option<&[Decoded]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Date(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Signed(value) => Some(*value),
            Self::Unsigned(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    }
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Data(value) => String::from_utf8_lossy(value).into_owned(),
            Self::String(value) | Self::Date(value) => value.clone(),
            Self::Null => String::new(),
            Self::Bool(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
            Self::Unsigned(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Array(_) | Self::Dict(_) => String::new(),
        }
    }
}

pub(crate) struct Decoder<'a> {
    objects: &'a [Value],
    active: HashSet<usize>,
}

impl<'a> Decoder<'a> {
    pub(crate) fn decode(&mut self, value: &Value) -> Result<Decoded> {
        match value {
            Value::Uid(uid) => self.decode_index(uid_to_index(uid)?),
            Value::Array(values) => values
                .iter()
                .map(|value| self.decode(value))
                .collect::<Result<Vec<_>>>()
                .map(Decoded::Array),
            Value::Dictionary(dict) => self.decode_plain_dict(dict),
            Value::Boolean(value) => Ok(Decoded::Bool(*value)),
            Value::Data(value) => Ok(Decoded::Data(value.clone())),
            Value::Date(value) => Ok(Decoded::Date(value.to_xml_format())),
            Value::Real(value) => Ok(Decoded::Real(*value)),
            Value::Integer(value) => {
                if let Some(value) = value.as_signed() {
                    Ok(Decoded::Signed(value))
                } else if let Some(value) = value.as_unsigned() {
                    Ok(Decoded::Unsigned(value))
                } else {
                    Err(SnipError::validation("unsupported plist integer"))
                }
            }
            Value::String(value) if value == "$null" => Ok(Decoded::Null),
            Value::String(value) => Ok(Decoded::String(value.clone())),
            _ => Err(SnipError::validation("unsupported plist value")),
        }
    }

    pub(crate) fn decode_index(&mut self, index: usize) -> Result<Decoded> {
        if index == 0 {
            return Ok(Decoded::Null);
        }
        let value = self
            .objects
            .get(index)
            .ok_or_else(|| SnipError::validation(format!("out-of-range archive UID {index}")))?;
        if !self.active.insert(index) {
            return Ok(Decoded::Null);
        }
        let result = match value {
            Value::Dictionary(dict) => self.decode_object_dict(dict),
            _ => self.decode(value),
        };
        self.active.remove(&index);
        result
    }

    fn decode_object_dict(&mut self, dict: &Dictionary) -> Result<Decoded> {
        let class = class_name(self.objects, dict)?;
        match class.as_deref() {
            Some("NSArray" | "NSMutableArray" | "NSSet" | "NSMutableSet") => dict
                .get("NS.objects")
                .and_then(Value::as_array)
                .ok_or_else(|| SnipError::validation("Foundation array is missing NS.objects"))?
                .iter()
                .map(|value| self.decode(value))
                .collect::<Result<Vec<_>>>()
                .map(Decoded::Array),
            Some("NSDictionary" | "NSMutableDictionary") => {
                let keys = dict
                    .get("NS.keys")
                    .and_then(Value::as_array)
                    .ok_or_else(|| SnipError::validation("NSDictionary is missing NS.keys"))?;
                let values = dict
                    .get("NS.objects")
                    .and_then(Value::as_array)
                    .ok_or_else(|| SnipError::validation("NSDictionary is missing NS.objects"))?;
                let mut result = BTreeMap::new();
                for (key, value) in keys.iter().zip(values) {
                    let key = self.decode(key)?;
                    let key = key
                        .as_str()
                        .ok_or_else(|| SnipError::validation("dictionary key is not text"))?;
                    result.insert(key.to_owned(), self.decode(value)?);
                }
                Ok(Decoded::Dict(result))
            }
            Some("NSData" | "NSMutableData") => match dict.get("NS.data") {
                Some(Value::Data(value)) => Ok(Decoded::Data(value.clone())),
                Some(value) => self.decode(value),
                None => Ok(Decoded::Data(Vec::new())),
            },
            Some("NSDate") => {
                let seconds = dict
                    .get("NS.time")
                    .and_then(number_value)
                    .ok_or_else(|| SnipError::validation("NSDate is missing NS.time"))?;
                Ok(Decoded::Date(ns_time_to_rfc3339(seconds)?))
            }
            _ => {
                let mut result = BTreeMap::new();
                if let Some(class) = class {
                    result.insert("__class".to_owned(), Decoded::String(class));
                }
                for (key, value) in dict {
                    if key != "$class" {
                        result.insert(key.clone(), self.decode(value)?);
                    }
                }
                Ok(Decoded::Dict(result))
            }
        }
    }

    fn decode_plain_dict(&mut self, dict: &Dictionary) -> Result<Decoded> {
        if dict.contains_key("$class") {
            return self.decode_object_dict(dict);
        }
        let mut result = BTreeMap::new();
        for (key, value) in dict {
            result.insert(key.clone(), self.decode(value)?);
        }
        Ok(Decoded::Dict(result))
    }
}

pub(crate) fn unarchive_bytes(data: &[u8]) -> Result<Decoded> {
    let value = Value::from_reader(Cursor::new(data))
        .map_err(|error| SnipError::validation(format!("invalid property list: {error}")))?;
    let archive = value
        .as_dictionary()
        .ok_or_else(|| SnipError::validation("archive root is not a dictionary"))?;
    if archive.get("$archiver").and_then(Value::as_string) != Some("NSKeyedArchiver") {
        return Err(SnipError::validation(
            "plist is not an NSKeyedArchiver archive",
        ));
    }
    let objects = archive
        .get("$objects")
        .and_then(Value::as_array)
        .ok_or_else(|| SnipError::validation("archive is missing $objects"))?;
    let root = archive
        .get("$top")
        .and_then(Value::as_dictionary)
        .and_then(|top| top.get("root"))
        .and_then(Value::as_uid)
        .ok_or_else(|| SnipError::validation("archive is missing root UID"))?;
    Decoder {
        objects,
        active: HashSet::new(),
    }
    .decode_index(uid_to_index(root)?)
}

pub(crate) fn class_name(objects: &[Value], dict: &Dictionary) -> Result<Option<String>> {
    let Some(Value::Uid(reference)) = dict.get("$class") else {
        return Ok(None);
    };
    let index = uid_to_index(reference)?;
    Ok(objects
        .get(index)
        .and_then(Value::as_dictionary)
        .and_then(|class| class.get("$classname"))
        .and_then(Value::as_string)
        .map(str::to_owned))
}

pub(crate) fn uid_to_index(uid: &Uid) -> Result<usize> {
    usize::try_from(uid.get()).map_err(|_| SnipError::validation("archive UID is too large"))
}

pub(crate) fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Real(value) => Some(*value),
        Value::Integer(value) => value
            .as_signed()
            .map(|value| value as f64)
            .or_else(|| value.as_unsigned().map(|value| value as f64)),
        _ => None,
    }
}

pub(crate) fn ns_time_to_rfc3339(seconds: f64) -> Result<String> {
    let nanos = ((seconds + APPLE_EPOCH_UNIX_SECONDS) * 1_000_000_000.0).round() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map_err(|error| SnipError::validation(format!("invalid NSDate: {error}")))?
        .format(&Rfc3339)
        .map_err(|error| SnipError::validation(format!("cannot format NSDate: {error}")))
}

pub(crate) fn decoded_string(dict: &BTreeMap<String, Decoded>, field: &str) -> Option<String> {
    dict.get(field).and_then(Decoded::as_str).map(str::to_owned)
}

pub(crate) fn required_string(dict: &BTreeMap<String, Decoded>, field: &str) -> Result<String> {
    decoded_string(dict, field)
        .ok_or_else(|| SnipError::validation(format!("missing string field {field}")))
}

pub(crate) fn parse_uuid(value: &str, kind: &str) -> Result<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| SnipError::validation(format!("invalid {kind} UUID {value:?}: {error}")))
}
