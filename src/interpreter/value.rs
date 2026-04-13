//! Runtime value types for the Fajar Lang interpreter.
//!
//! Defines the [`Value`] enum representing all possible runtime values,
//! and [`FnValue`] for function closures.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::parser::ast::{Expr, Param};
use crate::runtime::ml::TensorValue;
use crate::runtime::ml::quantize::QuantizedValue;

/// A lazy iterator value for the iterator protocol.
///
/// Iterators are consumed by calling `next()` repeatedly until `None`.
/// Combinators like `map`, `filter`, `take` wrap an inner iterator lazily.
#[derive(Debug, Clone)]
pub enum IteratorValue {
    /// Iterates over array elements.
    Array {
        /// Remaining elements.
        items: Vec<Value>,
        /// Current position.
        pos: usize,
    },
    /// Iterates over integer range.
    Range {
        /// Current value.
        current: i64,
        /// End value (exclusive).
        end: i64,
        /// Step (1 or -1).
        step: i64,
    },
    /// Iterates over string characters.
    Chars {
        /// Characters to iterate.
        chars: Vec<char>,
        /// Current position.
        pos: usize,
    },
    /// Iterates over map entries as (key, value) tuples.
    Map {
        /// Remaining entries.
        entries: Vec<(String, Value)>,
        /// Current position.
        pos: usize,
    },
    /// Lazy map combinator: applies a function to each element.
    MappedIter {
        /// Inner iterator.
        inner: Box<IteratorValue>,
        /// Function to apply.
        func: FnValue,
    },
    /// Lazy filter combinator: keeps elements matching predicate.
    FilterIter {
        /// Inner iterator.
        inner: Box<IteratorValue>,
        /// Predicate function.
        func: FnValue,
    },
    /// Lazy take combinator: yields at most N elements.
    TakeIter {
        /// Inner iterator.
        inner: Box<IteratorValue>,
        /// Remaining count.
        remaining: usize,
    },
    /// Lazy enumerate combinator: yields (index, element) tuples.
    EnumerateIter {
        /// Inner iterator.
        inner: Box<IteratorValue>,
        /// Current index.
        index: usize,
    },
}

impl IteratorValue {
    /// Advances the iterator and returns the next value, or None if exhausted.
    /// Note: combinators that need to call Fajar functions (map, filter)
    /// return a "needs-eval" marker — the interpreter handles those.
    pub fn next_simple(&mut self) -> Option<Value> {
        match self {
            IteratorValue::Array { items, pos } => {
                if *pos < items.len() {
                    let val = items[*pos].clone();
                    *pos += 1;
                    Some(val)
                } else {
                    None
                }
            }
            IteratorValue::Range { current, end, step } => {
                if (*step > 0 && *current < *end) || (*step < 0 && *current > *end) {
                    let val = *current;
                    *current += *step;
                    Some(Value::Int(val))
                } else {
                    None
                }
            }
            IteratorValue::Chars { chars, pos } => {
                if *pos < chars.len() {
                    let c = chars[*pos];
                    *pos += 1;
                    Some(Value::Char(c))
                } else {
                    None
                }
            }
            IteratorValue::Map { entries, pos } => {
                if *pos < entries.len() {
                    let (k, v) = entries[*pos].clone();
                    *pos += 1;
                    Some(Value::Tuple(vec![Value::Str(k), v]))
                } else {
                    None
                }
            }
            IteratorValue::TakeIter { inner, remaining } => {
                if *remaining == 0 {
                    return None;
                }
                *remaining -= 1;
                inner.next_simple()
            }
            IteratorValue::EnumerateIter { inner, index } => {
                if let Some(val) = inner.next_simple() {
                    let idx = *index;
                    *index += 1;
                    Some(Value::Tuple(vec![Value::Int(idx as i64), val]))
                } else {
                    None
                }
            }
            // MappedIter and FilterIter need the interpreter to call Fajar functions
            // They return None here — handled by the interpreter's iter_next() method
            IteratorValue::MappedIter { .. } | IteratorValue::FilterIter { .. } => None,
        }
    }
}

/// An opaque optimizer handle for SGD or Adam.
#[derive(Debug, Clone)]
pub enum OptimizerValue {
    /// Stochastic Gradient Descent.
    Sgd(crate::runtime::ml::optim::SGD),
    /// Adam optimizer.
    Adam(crate::runtime::ml::optim::Adam),
}

/// An opaque layer handle for neural network layers.
#[derive(Debug, Clone)]
pub enum LayerValue {
    /// Fully-connected (dense) layer.
    Dense(crate::runtime::ml::layers::Dense),
    /// 2D convolutional layer.
    Conv2d(crate::runtime::ml::layers::Conv2d),
    /// V18: Multi-head attention layer.
    Attention(Box<crate::runtime::ml::layers::MultiHeadAttention>),
}

/// A runtime value in the Fajar Lang interpreter.
///
/// Every expression evaluates to a `Value`. The interpreter operates
/// entirely on this enum.
///
/// # Examples
///
/// ```
/// use fajar_lang::interpreter::Value;
///
/// let v = Value::Int(42);
/// assert_eq!(format!("{v}"), "42");
///
/// let v = Value::Str("hello".into());
/// assert_eq!(format!("{v}"), "hello");
/// ```
#[derive(Debug, Clone)]
pub enum Value {
    /// The null value (for void expressions).
    Null,
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating point.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Unicode character.
    Char(char),
    /// String value.
    Str(String),
    /// Dynamically-sized array.
    Array(Vec<Value>),
    /// Fixed-size tuple.
    Tuple(Vec<Value>),
    /// Struct instance with named fields.
    Struct {
        /// Struct type name.
        name: String,
        /// Field name → value mapping.
        fields: HashMap<String, Value>,
    },
    /// Enum variant, optionally carrying data.
    Enum {
        /// Variant name (e.g., `"Circle"`).
        variant: String,
        /// Optional associated data.
        data: Option<Box<Value>>,
    },
    /// A user-defined function (closure).
    Function(FnValue),
    /// A built-in function referenced by name.
    BuiltinFn(String),
    /// A hash map with string keys.
    Map(HashMap<String, Value>),
    /// A raw memory address (OS runtime pointer).
    Pointer(u64),
    /// A tensor value (ML runtime).
    Tensor(TensorValue),
    /// A quantized tensor value (ML runtime, multi-bit).
    Quantized(QuantizedValue),
    /// An optimizer handle (ML runtime).
    Optimizer(OptimizerValue),
    /// A layer handle (ML runtime, boxed for size).
    Layer(Box<LayerValue>),
    /// A lazy iterator value.
    Iterator(Arc<Mutex<IteratorValue>>),
    /// An async future value (result of calling an async fn).
    Future {
        /// Unique task ID for the executor.
        task_id: u64,
    },
    /// V12: A generator value that yields values via resume().
    Generator {
        /// Generator name.
        name: String,
        /// Pre-computed values to yield.
        values: Vec<Value>,
        /// Current position in the value sequence.
        position: usize,
    },
    /// A trait object with dynamic dispatch via vtable.
    TraitObject {
        /// The trait name this object conforms to.
        trait_name: String,
        /// The concrete value wrapped as a trait object.
        concrete: Box<Value>,
        /// The concrete type name (for vtable lookup).
        concrete_type: String,
        /// Virtual method table: method name → function value.
        vtable: HashMap<String, FnValue>,
    },
}

/// A user-defined function value, capturing its closure environment.
///
/// Created when a `fn` definition or closure expression is evaluated.
#[derive(Debug, Clone)]
pub struct FnValue {
    /// Function name (empty string for anonymous closures).
    pub name: String,
    /// Parameter definitions.
    pub params: Vec<Param>,
    /// Function body expression (typically a Block).
    pub body: Box<Expr>,
    /// Captured environment at the point of definition.
    pub closure_env: crate::interpreter::env::EnvRef,
    /// Whether this is an async function (returns Future on call).
    pub is_async: bool,
    /// Whether this is a generator function (`gen fn`).
    pub is_gen: bool,
    /// V18: @requires precondition expressions (evaluated at call time).
    pub requires: Vec<Box<crate::parser::ast::Expr>>,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (
                Value::Struct {
                    name: n1,
                    fields: f1,
                },
                Value::Struct {
                    name: n2,
                    fields: f2,
                },
            ) => n1 == n2 && f1 == f2,
            (
                Value::Enum {
                    variant: v1,
                    data: d1,
                },
                Value::Enum {
                    variant: v2,
                    data: d2,
                },
            ) => v1 == v2 && d1 == d2,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::BuiltinFn(a), Value::BuiltinFn(b)) => a == b,
            (Value::Pointer(a), Value::Pointer(b)) => a == b,
            (Value::Tensor(a), Value::Tensor(b)) => a == b,
            (Value::Quantized(a), Value::Quantized(b)) => a == b,
            // Iterators are never equal (stateful)
            (Value::Iterator(_), Value::Iterator(_)) => false,
            // TraitObjects: compare by concrete value
            (
                Value::TraitObject {
                    concrete: c1,
                    trait_name: t1,
                    ..
                },
                Value::TraitObject {
                    concrete: c2,
                    trait_name: t2,
                    ..
                },
            ) => t1 == t2 && c1 == c2,
            // Futures are equal if same task ID
            (Value::Future { task_id: a }, Value::Future { task_id: b }) => a == b,
            (Value::Generator { name: a, .. }, Value::Generator { name: b, .. }) => a == b,
            // Functions, optimizers, layers are never equal (no structural comparison)
            (Value::Function(_), Value::Function(_)) => false,
            (Value::Optimizer(_), Value::Optimizer(_)) => false,
            (Value::Layer(_), Value::Layer(_)) => false,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Char(c) => write!(f, "{c}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Array(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, "]")
            }
            Value::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ")")
            }
            Value::Struct { name, fields } => {
                write!(f, "{name} {{ ")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            Value::Enum { variant, data } => {
                write!(f, "{variant}")?;
                if let Some(d) = data {
                    write!(f, "({d})")?;
                }
                Ok(())
            }
            Value::Function(fv) => {
                if fv.name.is_empty() {
                    write!(f, "<closure>")
                } else {
                    write!(f, "<fn {}>", fv.name)
                }
            }
            Value::BuiltinFn(name) => write!(f, "<builtin {name}>"),
            Value::Map(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{k}\": {v}")?;
                }
                write!(f, "}}")
            }
            Value::Pointer(addr) => write!(f, "0x{addr:08x}"),
            Value::Tensor(t) => write!(f, "{t}"),
            Value::Quantized(q) => write!(f, "{q}"),
            Value::Optimizer(o) => match o {
                OptimizerValue::Sgd(_) => write!(f, "<optimizer SGD>"),
                OptimizerValue::Adam(_) => write!(f, "<optimizer Adam>"),
            },
            Value::Layer(l) => match l.as_ref() {
                LayerValue::Dense(_) => write!(f, "<layer Dense>"),
                LayerValue::Conv2d(_) => write!(f, "<layer Conv2d>"),
                LayerValue::Attention(_) => write!(f, "<layer MultiHeadAttention>"),
            },
            Value::Iterator(_) => write!(f, "<iterator>"),
            Value::Future { task_id } => write!(f, "<future:{task_id}>"),
            Value::Generator {
                name,
                values,
                position,
            } => {
                write!(f, "<generator:{name} [{position}/{}]>", values.len())
            }
            Value::TraitObject {
                trait_name,
                concrete_type,
                ..
            } => write!(f, "<dyn {trait_name} ({concrete_type})>"),
        }
    }
}

impl Value {
    /// Returns `true` if this value is truthy.
    ///
    /// Truthiness rules:
    /// - `Bool(false)` → falsy
    /// - `Null` → falsy
    /// - `Int(0)` → falsy
    /// - Everything else → truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Int(0) => false,
            _ => true,
        }
    }

    /// Returns the type name of this value as a string.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Int(_) => "i64",
            Value::Float(_) => "f64",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Str(_) => "str",
            Value::Array(_) => "array",
            Value::Tuple(_) => "tuple",
            Value::Map(_) => "map",
            Value::Struct { .. } => "struct",
            Value::Enum { .. } => "enum",
            Value::Function(_) => "function",
            Value::BuiltinFn(_) => "builtin",
            Value::Pointer(_) => "pointer",
            Value::Tensor(_) => "tensor",
            Value::Quantized(_) => "quantized",
            Value::Optimizer(_) => "optimizer",
            Value::Layer(_) => "layer",
            Value::Iterator(_) => "iterator",
            Value::Future { .. } => "future",
            Value::Generator { .. } => "generator",
            Value::TraitObject { .. } => "trait_object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_null_display() {
        assert_eq!(format!("{}", Value::Null), "null");
    }

    #[test]
    fn value_int_display() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
    }

    #[test]
    fn value_float_display() {
        assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
    }

    #[test]
    fn value_bool_display() {
        assert_eq!(format!("{}", Value::Bool(true)), "true");
        assert_eq!(format!("{}", Value::Bool(false)), "false");
    }

    #[test]
    fn value_char_display() {
        assert_eq!(format!("{}", Value::Char('a')), "a");
    }

    #[test]
    fn value_str_display() {
        assert_eq!(format!("{}", Value::Str("hello".into())), "hello");
    }

    #[test]
    fn value_array_display() {
        let arr = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(format!("{arr}"), "[1, 2, 3]");
    }

    #[test]
    fn value_tuple_display() {
        let t = Value::Tuple(vec![Value::Int(1), Value::Str("hi".into())]);
        assert_eq!(format!("{t}"), "(1, hi)");
    }

    #[test]
    fn value_enum_display() {
        let e = Value::Enum {
            variant: "Circle".into(),
            data: Some(Box::new(Value::Float(5.0))),
        };
        assert_eq!(format!("{e}"), "Circle(5)");
    }

    #[test]
    fn value_enum_unit_display() {
        let e = Value::Enum {
            variant: "None".into(),
            data: None,
        };
        assert_eq!(format!("{e}"), "None");
    }

    #[test]
    fn value_builtin_fn_display() {
        assert_eq!(
            format!("{}", Value::BuiltinFn("println".into())),
            "<builtin println>"
        );
    }

    #[test]
    fn value_equality_same_types() {
        assert_eq!(Value::Null, Value::Null);
        assert_eq!(Value::Int(42), Value::Int(42));
        assert_eq!(Value::Float(1.0), Value::Float(1.0));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Char('x'), Value::Char('x'));
        assert_eq!(Value::Str("hi".into()), Value::Str("hi".into()));
    }

    #[test]
    fn value_inequality_different_values() {
        assert_ne!(Value::Int(1), Value::Int(2));
        assert_ne!(Value::Bool(true), Value::Bool(false));
        assert_ne!(Value::Str("a".into()), Value::Str("b".into()));
    }

    #[test]
    fn value_inequality_different_types() {
        assert_ne!(Value::Int(1), Value::Float(1.0));
        assert_ne!(Value::Int(0), Value::Bool(false));
        assert_ne!(Value::Null, Value::Int(0));
    }

    #[test]
    fn value_is_truthy() {
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(Value::Int(-1).is_truthy());
        assert!(Value::Str("hello".into()).is_truthy());
        assert!(Value::Array(vec![]).is_truthy());

        assert!(!Value::Bool(false).is_truthy());
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Int(0).is_truthy());
    }

    #[test]
    fn value_type_name() {
        assert_eq!(Value::Null.type_name(), "null");
        assert_eq!(Value::Int(0).type_name(), "i64");
        assert_eq!(Value::Float(0.0).type_name(), "f64");
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(Value::Char('a').type_name(), "char");
        assert_eq!(Value::Str("".into()).type_name(), "str");
        assert_eq!(Value::Array(vec![]).type_name(), "array");
        assert_eq!(Value::Tuple(vec![]).type_name(), "tuple");
        assert_eq!(Value::BuiltinFn("x".into()).type_name(), "builtin");
    }

    #[test]
    fn value_array_equality() {
        let a = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let b = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let c = Value::Array(vec![Value::Int(1), Value::Int(3)]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn value_struct_equality() {
        let mut f1 = HashMap::new();
        f1.insert("x".into(), Value::Int(1));
        let mut f2 = HashMap::new();
        f2.insert("x".into(), Value::Int(1));

        let s1 = Value::Struct {
            name: "Point".into(),
            fields: f1,
        };
        let s2 = Value::Struct {
            name: "Point".into(),
            fields: f2,
        };
        assert_eq!(s1, s2);
    }
}
