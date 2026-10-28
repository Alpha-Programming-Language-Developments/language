#![allow(dead_code)]
// Dynamic runtime values for the Alpha programming language.
use crate::frontend::ast::{Block, FunctionDecl};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct IoFileHandle {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FsDirHandle {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StructureInstance {
    pub name: String,
    pub fields: Rc<RefCell<HashMap<String, Value>>>,
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Unt(u64),
    Float(f64),
    String(String),
    Char(char),
    Boolean(bool),
    Nothing,
    Vect(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<HashMap<ValueKey, Value>>>),
    IoFile(Rc<RefCell<IoFileHandle>>),
    FsDir(Rc<RefCell<FsDirHandle>>),
    BuiltinNamespace(String), // "Io", "Fs", "env", "dir.file"
    Function(FunctionDecl),
    StructureInstance(StructureInstance),
    Tuple(Vec<Value>),
    HexColor(String),
    Block(Block),
    EnumVariant {
        enum_name: String,
        variant: String,
        ordinal: usize,
    },
}

/// Key type for Map values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKey {
    Int(i64),
    Unt(u64),
    String(String),
    Char(char),
    Boolean(bool),
}

impl Hash for ValueKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ValueKey::Int(i) => {
                0u8.hash(state);
                i.hash(state);
            }
            ValueKey::String(s) => {
                1u8.hash(state);
                s.hash(state);
            }
            ValueKey::Char(c) => {
                2u8.hash(state);
                c.hash(state);
            }
            ValueKey::Boolean(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            ValueKey::Unt(u) => {
                4u8.hash(state);
                u.hash(state);
            }
        }
    }
}

impl std::fmt::Display for ValueKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueKey::Int(i) => write!(f, "{}", i),
            ValueKey::Unt(u) => write!(f, "{}", u),
            ValueKey::String(s) => write!(f, "{}", s),
            ValueKey::Char(c) => write!(f, "{}", c),
            ValueKey::Boolean(b) => write!(f, "{}", b),
        }
    }
}

impl Value {
    pub fn is_nothing(&self) -> bool {
        matches!(self, Value::Nothing)
    }

    pub fn as_unt(&self) -> Option<u64> {
        match self {
            Value::Unt(u) => Some(*u),
            Value::Int(i) if *i >= 0 => Some(*i as u64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn to_key(&self) -> Result<ValueKey, String> {
        match self {
            Value::Int(i) => Ok(ValueKey::Int(*i)),
            Value::Unt(u) => Ok(ValueKey::Unt(*u)),
            Value::String(s) => Ok(ValueKey::String(s.clone())),
            Value::Char(c) => Ok(ValueKey::Char(*c)),
            Value::Boolean(b) => Ok(ValueKey::Boolean(*b)),
            other => Err(format!("Type '{}' cannot be used as map key", other.type_name())),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Unt(_) => "unt",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Char(_) => "char",
            Value::Boolean(_) => "boolean",
            Value::Nothing => "nothing",
            Value::Vect(_) => "vect",
            Value::Map(_) => "map",
            Value::IoFile(_) => "Io.File",
            Value::FsDir(_) => "Fs.Dir",
            Value::BuiltinNamespace(_) => "namespace",
            Value::Function(_) => "function",
            Value::StructureInstance(_) => "structure",
            Value::Tuple(_) => "tuple",
            Value::HexColor(_) => "hexcolor",
            Value::Block(_) => "block",
            Value::EnumVariant { .. } => "enum",
        }
    }

    pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
        let clean = hex.trim().trim_start_matches('#');
        if clean.len() == 3 {
            let r = u8::from_str_radix(&clean[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[2..3].repeat(2), 16).unwrap_or(0);
            (r, g, b)
        } else if clean.len() == 5 {
            // E.g. FFF00 -> FF, F0, 00
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&format!("{}0", &clean[4..5]), 16).unwrap_or(0);
            (r, g, b)
        } else if clean.len() >= 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0);
            (r, g, b)
        } else {
            (255, 255, 255)
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            Value::Nothing => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            _ => true,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Unt(a), Value::Unt(b)) => a == b,
            (Value::Int(a), Value::Unt(b)) => *a >= 0 && (*a as u64) == *b,
            (Value::Unt(a), Value::Int(b)) => *b >= 0 && *a == (*b as u64),
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Int(a), Value::Float(b)) => (*a as f64 - b).abs() < f64::EPSILON,
            (Value::Float(a), Value::Int(b)) => (a - *b as f64).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Nothing, Value::Nothing) => true,
            (Value::StructureInstance(a), Value::StructureInstance(b)) => {
                a.name == b.name && Rc::ptr_eq(&a.fields, &b.fields)
            }
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::HexColor(a), Value::HexColor(b)) => a.eq_ignore_ascii_case(b),
            (
                Value::EnumVariant { enum_name: e1, variant: v1, .. },
                Value::EnumVariant { enum_name: e2, variant: v2, .. },
            ) => e1 == e2 && v1 == v2,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Unt(u) => write!(f, "{}", u),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s),
            Value::Char(c) => write!(f, "{}", c),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Nothing => write!(f, "nothing"),
            Value::Vect(v) => {
                let items = v.borrow();
                let strs: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", strs.join(", "))
            }
            Value::Map(m) => {
                let map = m.borrow();
                let mut entries = Vec::new();
                for (k, v) in map.iter() {
                    let k_str = match k {
                        ValueKey::Int(i) => i.to_string(),
                        ValueKey::Unt(u) => u.to_string(),
                        ValueKey::String(s) => format!("\"{}\"", s),
                        ValueKey::Char(c) => format!("'{}'", c),
                        ValueKey::Boolean(b) => b.to_string(),
                    };
                    entries.push(format!("{}: {}", k_str, v));
                }
                write!(f, "{{{}}}", entries.join(", "))
            }
            Value::IoFile(handle) => write!(f, "Io.File({:?})", handle.borrow().path),
            Value::FsDir(handle) => write!(f, "Fs.Dir({:?})", handle.borrow().path),
            Value::BuiltinNamespace(name) => write!(f, "<namespace {}>", name),
            Value::Function(func) => write!(f, "<function {}>", func.name),
            Value::StructureInstance(inst) => {
                let fields = inst.fields.borrow();
                let mut entries = Vec::new();
                for (k, v) in fields.iter() {
                    entries.push(format!("{}: {}", k, v));
                }
                write!(f, "{}{{{}}}", inst.name, entries.join(", "))
            }
            Value::Tuple(items) => {
                let strs: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                write!(f, "({})", strs.join(", "))
            }
            Value::HexColor(h) => write!(f, "#({})", h),
            Value::EnumVariant { enum_name, variant, .. } => write!(f, "{}.{}", enum_name, variant),
            Value::Block(_) => write!(f, "<block>"),
        }
    }
}
