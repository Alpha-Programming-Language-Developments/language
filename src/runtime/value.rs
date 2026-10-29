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
pub struct ArenaSectorData {
    pub id: usize,
    pub generation: usize,
    pub storage: Vec<Value>,
    pub allocated_bytes: usize,
}

impl ArenaSectorData {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            generation: 1,
            storage: Vec::new(),
            allocated_bytes: 0,
        }
    }

    pub fn insert_value(&mut self, val: Value) -> usize {
        let size = val.estimated_size_bytes();
        self.allocated_bytes += size;
        let slot = self.storage.len();
        self.storage.push(val);
        slot
    }

    pub fn reset(&mut self) {
        self.storage.clear();
        self.allocated_bytes = 0;
        self.generation += 1;
    }
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
    BuiltinNamespace(String), // "Io", "Fs", "env", "dir.file", "mem"
    Function(FunctionDecl),
    StructureInstance(StructureInstance),
    Tuple(Vec<Value>),
    HexColor(String),
    Block(Block),
    ArenaSector(Rc<RefCell<ArenaSectorData>>),
    ArenaRef {
        sector: Rc<RefCell<ArenaSectorData>>,
        slot: usize,
        generation: usize,
    },
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
            Value::ArenaSector(_) => "Mem.ArenaSector",
            Value::ArenaRef { .. } => "arena_ref",
            Value::EnumVariant { .. } => "enum",
        }
    }

    pub fn estimated_size_bytes(&self) -> usize {
        match self {
            Value::Int(_) | Value::Unt(_) | Value::Float(_) | Value::Boolean(_) | Value::Char(_) => 8,
            Value::Nothing => 0,
            Value::String(s) => s.len() + 24,
            Value::HexColor(h) => h.len() + 24,
            Value::Tuple(t) => t.len() * 16 + 16,
            Value::Vect(v) => v.borrow().len() * 16 + 24,
            Value::Map(m) => m.borrow().len() * 48 + 32,
            Value::StructureInstance(s) => s.fields.borrow().len() * 32 + 32,
            Value::ArenaRef { .. } => 24,
            _ => 16,
        }
    }

    pub fn deref_arena(&self, span: crate::diagnostics::error::Span) -> Result<Value, crate::diagnostics::error::AlphaError> {
        match self {
            Value::ArenaRef { sector, slot, generation } => {
                let sec = sector.borrow();
                if sec.generation != *generation || *slot >= sec.storage.len() {
                    return Err(crate::diagnostics::error::AlphaError::runtime(
                        "Arena memory error: Object was deallocated when arena sector was reset (Use-After-Free prevented)",
                        span,
                    ));
                }
                sec.storage[*slot].deref_arena(span)
            }
            other => Ok(other.clone()),
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
            (Value::ArenaSector(a), Value::ArenaSector(b)) => Rc::ptr_eq(a, b) || a.borrow().id == b.borrow().id,
            (
                Value::ArenaRef { sector: s1, slot: sl1, generation: g1 },
                Value::ArenaRef { sector: s2, slot: sl2, generation: g2 },
            ) => Rc::ptr_eq(s1, s2) && sl1 == sl2 && g1 == g2,
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
            Value::ArenaSector(sec) => write!(f, "Mem.ArenaSector(id={}, count={}, bytes={})", sec.borrow().id, sec.borrow().storage.len(), sec.borrow().allocated_bytes),
            Value::ArenaRef { sector, slot, generation } => {
                let sec = sector.borrow();
                if sec.generation == *generation && *slot < sec.storage.len() {
                    write!(f, "{}", sec.storage[*slot])
                } else {
                    write!(f, "<deallocated arena object>")
                }
            }
            Value::EnumVariant { enum_name, variant, .. } => write!(f, "{}.{}", enum_name, variant),
            Value::Block(_) => write!(f, "<block>"),
        }
    }
}
