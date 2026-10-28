#![allow(dead_code)]
// Type representation and type compatibility checking for Alpha.
use std::fmt;

/// Representation of types in the Alpha language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    String,
    Char,
    Boolean,
    HexColor,
    Something, // Any type except nothing
    Nothing,   // Absence of value
    Vect(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Union(Vec<Type>),
    Custom(String), // e.g. "Io.File", "Fs.Dir"
}

impl Type {
    /// Check whether a source type can be assigned to or matches a target type.
    pub fn is_assignable_to(&self, target: &Type) -> bool {
        if self == target {
            return true;
        }

        // 'something' as target accepts anything except 'nothing'
        if *target == Type::Something {
            return *self != Type::Nothing;
        }

        // 'something' as source can be dynamically allowed if permissive
        if *self == Type::Something && *target != Type::Nothing {
            return true;
        }

        // Target is a Union: e.g. <int, nothing>
        if let Type::Union(variants) = target {
            return variants.iter().any(|v| self.is_assignable_to(v));
        }

        // Source is a Union: all variants must be assignable to target
        if let Type::Union(variants) = self {
            return variants.iter().all(|v| v.is_assignable_to(target));
        }

        // Vectors
        if let (Type::Vect(inner_src), Type::Vect(inner_dst)) = (self, target) {
            return inner_src.is_assignable_to(inner_dst);
        }

        // Maps
        if let (Type::Map(k_src, v_src), Type::Map(k_dst, v_dst)) = (self, target) {
            return k_src.is_assignable_to(k_dst) && v_src.is_assignable_to(v_dst);
        }

        // Tuples
        if let (Type::Tuple(srcs), Type::Tuple(dsts)) = (self, target) {
            if srcs.len() != dsts.len() {
                return false;
            }
            return srcs.iter().zip(dsts).all(|(s, d)| s.is_assignable_to(d));
        }

        false
    }

    /// Check if this type allows 'nothing'.
    pub fn allows_nothing(&self) -> bool {
        match self {
            Type::Nothing => true,
            Type::Union(variants) => variants.iter().any(|v| v.allows_nothing()),
            _ => false,
        }
    }

    /// Returns the single non-nothing type if this is a union like `<T, nothing>`, otherwise returns self.
    pub fn unwrapped_non_nothing(&self) -> &Type {
        match self {
            Type::Union(variants) => {
                let non_nothing: Vec<&Type> = variants.iter().filter(|t| **t != Type::Nothing).collect();
                if non_nothing.len() == 1 {
                    non_nothing[0]
                } else {
                    self
                }
            }
            other => other,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "string"),
            Type::Char => write!(f, "char"),
            Type::Boolean => write!(f, "boolean"),
            Type::HexColor => write!(f, "hexcolor"),
            Type::Something => write!(f, "something"),
            Type::Nothing => write!(f, "nothing"),
            Type::Vect(inner) => write!(f, "vect<{}>", inner),
            Type::Map(k, v) => write!(f, "map<{}, {}>", k, v),
            Type::Tuple(types) => {
                let s: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", s.join(", "))
            }
            Type::Union(types) => {
                let s: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                write!(f, "<{}>", s.join(", "))
            }
            Type::Custom(name) => write!(f, "{}", name),
        }
    }
}
