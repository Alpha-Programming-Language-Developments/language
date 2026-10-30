#![allow(dead_code)]
// Runtime variable environment for the Alpha language.
use crate::frontend::ast::Visibility;
use crate::runtime::value::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Binding {
    pub value: Value,
    pub is_const: bool,
    pub visibility: Visibility,
    pub source_file: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct ScopeFrame {
    bindings: HashMap<String, Binding>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    frames: Vec<ScopeFrame>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            frames: vec![ScopeFrame::default()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.frames.push(ScopeFrame::default());
    }

    pub fn exit_scope(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
    }

    pub fn define(&mut self, name: impl Into<String>, value: Value, is_const: bool) {
        let frame = self.frames.last_mut().expect("At least one scope frame");
        frame.bindings.insert(name.into(), Binding {
            value,
            is_const,
            visibility: Visibility::Public,
            source_file: None,
        });
    }

    pub fn define_with_visibility(
        &mut self,
        name: impl Into<String>,
        value: Value,
        is_const: bool,
        visibility: Visibility,
        source_file: Option<String>,
    ) {
        let frame = self.frames.last_mut().expect("At least one scope frame");
        frame.bindings.insert(name.into(), Binding {
            value,
            is_const,
            visibility,
            source_file,
        });
    }

    pub fn get_binding(&self, name: &str) -> Option<&Binding> {
        for frame in self.frames.iter().rev() {
            if let Some(b) = frame.bindings.get(name) {
                return Some(b);
            }
        }
        let clean = name.replace('?', "");
        if !clean.is_empty() {
            for frame in self.frames.iter().rev() {
                if let Some(b) = frame.bindings.get(&clean) {
                    return Some(b);
                }
                for (k, b) in &frame.bindings {
                    if k.replace('?', "") == clean {
                        return Some(b);
                    }
                }
            }
        }
        None
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.get_binding(name).map(|b| b.value.clone())
    }

    pub fn assign(&mut self, name: &str, value: Value) -> Result<(), String> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(b) = frame.bindings.get_mut(name) {
                if b.is_const {
                    return Err(format!("Cannot reassign constant variable '{}'", name));
                }
                b.value = value;
                return Ok(());
            }
        }
        let clean = name.replace('?', "");
        if !clean.is_empty() {
            for frame in self.frames.iter_mut().rev() {
                if let Some(b) = frame.bindings.get_mut(&clean) {
                    if b.is_const {
                        return Err(format!("Cannot reassign constant variable '{}'", clean));
                    }
                    b.value = value;
                    return Ok(());
                }
                let found_key = frame.bindings.keys().find(|k| k.replace('?', "") == clean).cloned();
                if let Some(key) = found_key {
                    let b = frame.bindings.get_mut(&key).unwrap();
                    if b.is_const {
                        return Err(format!("Cannot reassign constant variable '{}'", key));
                    }
                    b.value = value;
                    return Ok(());
                }
            }
        }
        Err(format!("Undefined variable '{}'", name))
    }
}
