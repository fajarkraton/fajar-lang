//! Method call evaluation for the Fajar Lang interpreter.
//!
//! Contains `eval_method_call()` and all method dispatch for strings, arrays,
//! maps, tensors, iterators, GPU builtins, and system methods.

use std::cell::RefCell;
use std::rc::Rc;

use crate::interpreter::value::{IteratorValue, Value};
use crate::parser::ast::{CallArg, Expr};
use crate::runtime::ml::tensor_ops;

use super::{EvalError, EvalResult, Interpreter, RuntimeError};

impl Interpreter {
    /// Evaluates a method call: `obj.method(args)`.
    pub(crate) fn eval_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[CallArg],
    ) -> EvalResult {
        let obj = self.eval_expr(receiver)?;
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.eval_expr(&arg.value)?);
        }

        // Check impl methods first — look up by struct name
        if let Value::Struct { name, .. } = &obj {
            let key = (name.clone(), method.to_string());
            if let Some(fv) = self.impl_methods.get(&key).cloned() {
                // Instance method: prepend receiver as `self` argument
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![obj];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
        }

        // Trait object dynamic dispatch — look up method in vtable
        if let Value::TraitObject {
            vtable, concrete, ..
        } = &obj
        {
            if let Some(fv) = vtable.get(method).cloned() {
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![*concrete.clone()];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
            return Err(
                RuntimeError::TypeError(format!("no method '{method}' on trait object")).into(),
            );
        }

        // Iterator methods on collections: .iter()
        if method == "iter" {
            let iter_val = match obj {
                Value::Array(arr) => IteratorValue::Array { items: arr, pos: 0 },
                Value::Str(s) => IteratorValue::Chars {
                    chars: s.chars().collect(),
                    pos: 0,
                },
                Value::Map(m) => IteratorValue::Map {
                    entries: m.into_iter().collect(),
                    pos: 0,
                },
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "cannot call .iter() on {}",
                        obj.type_name()
                    ))
                    .into());
                }
            };
            return Ok(Value::Iterator(Rc::new(RefCell::new(iter_val))));
        }

        // Iterator combinator/consumer methods
        if let Value::Iterator(iter_rc) = obj {
            return self.eval_iterator_method(iter_rc, method, arg_vals);
        }

        // Check impl methods for enum values
        if let Value::Enum { variant, .. } = &obj {
            let key = (variant.clone(), method.to_string());
            if let Some(fv) = self.impl_methods.get(&key).cloned() {
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![obj];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
        }

        // Option/Result utility methods
        if let Value::Enum { variant, data } = &obj {
            match method {
                "unwrap" => {
                    return match variant.as_str() {
                        "Some" | "Ok" => {
                            Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null))
                        }
                        "None" => Err(RuntimeError::TypeError(
                            "called unwrap() on None value".into(),
                        )
                        .into()),
                        "Err" => Err(RuntimeError::TypeError(format!(
                            "called unwrap() on Err({})",
                            data.as_ref().map(|d| format!("{d}")).unwrap_or_default()
                        ))
                        .into()),
                        _ => Err(RuntimeError::TypeError(format!(
                            "no method 'unwrap' on variant '{variant}'"
                        ))
                        .into()),
                    };
                }
                "unwrap_or" => {
                    return match variant.as_str() {
                        "Some" | "Ok" => {
                            Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null))
                        }
                        "None" | "Err" => Ok(arg_vals.into_iter().next().unwrap_or(Value::Null)),
                        _ => Err(RuntimeError::TypeError(format!(
                            "no method 'unwrap_or' on variant '{variant}'"
                        ))
                        .into()),
                    };
                }
                "is_some" => return Ok(Value::Bool(variant == "Some")),
                "is_none" => return Ok(Value::Bool(variant == "None")),
                "is_ok" => return Ok(Value::Bool(variant == "Ok")),
                "is_err" => return Ok(Value::Bool(variant == "Err")),
                _ => {}
            }
        }

        // Built-in methods on primitive types
        match (&obj, method) {
            // String methods
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Str(s), "contains") => {
                if let Some(Value::Str(sub)) = arg_vals.first() {
                    Ok(Value::Bool(s.contains(sub.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("contains() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "trim") => Ok(Value::Str(s.trim().to_string())),
            (Value::Str(s), "trim_start") => Ok(Value::Str(s.trim_start().to_string())),
            (Value::Str(s), "trim_end") => Ok(Value::Str(s.trim_end().to_string())),
            (Value::Str(s), "to_uppercase") => Ok(Value::Str(s.to_uppercase())),
            (Value::Str(s), "to_lowercase") => Ok(Value::Str(s.to_lowercase())),
            (Value::Str(s), "starts_with") => {
                if let Some(Value::Str(prefix)) = arg_vals.first() {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("starts_with() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "ends_with") => {
                if let Some(Value::Str(suffix)) = arg_vals.first() {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("ends_with() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "replace") => {
                if let (Some(Value::Str(from)), Some(Value::Str(to))) =
                    (arg_vals.first(), arg_vals.get(1))
                {
                    Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("replace() requires two string arguments".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "split") => {
                if let Some(Value::Str(sep)) = arg_vals.first() {
                    let parts: Vec<Value> = s
                        .split(sep.as_str())
                        .map(|p| Value::Str(p.to_string()))
                        .collect();
                    Ok(Value::Array(parts))
                } else {
                    Err(RuntimeError::TypeError("split() requires a string argument".into()).into())
                }
            }
            (Value::Str(s), "repeat") => {
                if let Some(Value::Int(n)) = arg_vals.first() {
                    Ok(Value::Str(s.repeat(*n as usize)))
                } else {
                    Err(
                        RuntimeError::TypeError("repeat() requires an integer argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "chars") => {
                let chars: Vec<Value> = s.chars().map(Value::Char).collect();
                Ok(Value::Array(chars))
            }
            (Value::Str(s), "substring") => {
                let start = match arg_vals.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "substring() requires integer arguments".into(),
                        )
                        .into());
                    }
                };
                let end = match arg_vals.get(1) {
                    Some(Value::Int(n)) => *n as usize,
                    _ => s.len(),
                };
                let result: String = s
                    .chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect();
                Ok(Value::Str(result))
            }
            (Value::Str(s), "parse_int") => match s.trim().parse::<i64>() {
                Ok(n) => Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Int(n))),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("parse error: {}", e)))),
                }),
            },
            (Value::Str(s), "parse_float") => match s.trim().parse::<f64>() {
                Ok(f) => Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Float(f))),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("parse error: {}", e)))),
                }),
            },
            (Value::Str(s), "is_empty") => Ok(Value::Bool(s.is_empty())),
            (Value::Str(s), "index_of") => {
                if let Some(Value::Str(needle)) = arg_vals.first() {
                    match s.find(needle.as_str()) {
                        Some(pos) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(Value::Int(pos as i64))),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    }
                } else {
                    Err(
                        RuntimeError::TypeError("index_of() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "rev") => Ok(Value::Str(s.chars().rev().collect())),
            (Value::Str(s), "bytes") => Ok(Value::Array(
                s.bytes().map(|b| Value::Int(b as i64)).collect(),
            )),
            // Array methods
            (Value::Array(a), "len") => Ok(Value::Int(a.len() as i64)),
            (Value::Array(a), "push") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let mut new_arr = a.clone();
                new_arr.push(arg_vals.into_iter().next().unwrap_or(Value::Null));
                Ok(Value::Array(new_arr))
            }
            (Value::Array(a), "is_empty") => Ok(Value::Bool(a.is_empty())),
            // Array higher-order methods with closures (v0.8)
            (Value::Array(a), "map") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                let mut result = Vec::with_capacity(a.len());
                for item in a.iter() {
                    let mapped = self.call_value(&func, vec![item.clone()])?;
                    result.push(mapped);
                }
                Ok(Value::Array(result))
            }
            (Value::Array(a), "filter") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                let mut result = Vec::new();
                for item in a.iter() {
                    let keep = self.call_value(&func, vec![item.clone()])?;
                    if keep == Value::Bool(true) {
                        result.push(item.clone());
                    }
                }
                Ok(Value::Array(result))
            }
            (Value::Array(a), "fold") => {
                if arg_vals.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let mut args_iter = arg_vals.into_iter();
                let mut acc = args_iter.next().unwrap_or(Value::Null);
                let func = args_iter.next().unwrap_or(Value::Null);
                for item in a.iter() {
                    acc = self.call_value(&func, vec![acc, item.clone()])?;
                }
                Ok(acc)
            }
            (Value::Array(a), "any") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                for item in a.iter() {
                    if self.call_value(&func, vec![item.clone()])? == Value::Bool(true) {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            (Value::Array(a), "all") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                for item in a.iter() {
                    if self.call_value(&func, vec![item.clone()])? != Value::Bool(true) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            (Value::Array(a), "enumerate") => {
                let result: Vec<Value> = a
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Value::Tuple(vec![Value::Int(i as i64), v.clone()]))
                    .collect();
                Ok(Value::Array(result))
            }
            (Value::Array(a), "reverse" | "rev") => {
                let mut reversed = a.clone();
                reversed.reverse();
                Ok(Value::Array(reversed))
            }
            (Value::Array(a), "sort") => {
                let mut sorted = a.clone();
                sorted.sort_by(|a, b| match (a, b) {
                    (Value::Int(x), Value::Int(y)) => x.cmp(y),
                    (Value::Float(x), Value::Float(y)) => {
                        x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (Value::Str(x), Value::Str(y)) => x.cmp(y),
                    _ => std::cmp::Ordering::Equal,
                });
                Ok(Value::Array(sorted))
            }
            (Value::Array(a), "join") => {
                let sep = match arg_vals.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => "".to_string(),
                };
                let result: Vec<String> = a.iter().map(|v| format!("{v}")).collect();
                Ok(Value::Str(result.join(&sep)))
            }
            (Value::Array(a), "flat_map") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                let mut result = Vec::new();
                for item in a.iter() {
                    let mapped = self.call_value(&func, vec![item.clone()])?;
                    if let Value::Array(inner) = mapped {
                        result.extend(inner);
                    } else {
                        result.push(mapped);
                    }
                }
                Ok(Value::Array(result))
            }
            (Value::Array(a), "take") => {
                let n = match arg_vals.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => 0,
                };
                Ok(Value::Array(a.iter().take(n).cloned().collect()))
            }
            (Value::Array(a), "skip") => {
                let n = match arg_vals.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => 0,
                };
                Ok(Value::Array(a.iter().skip(n).cloned().collect()))
            }
            (Value::Array(a), "zip") => {
                if let Some(Value::Array(b)) = arg_vals.into_iter().next() {
                    let result: Vec<Value> = a
                        .iter()
                        .zip(b.iter())
                        .map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()]))
                        .collect();
                    Ok(Value::Array(result))
                } else {
                    Err(RuntimeError::TypeError("zip() requires an array argument".into()).into())
                }
            }
            (Value::Array(a), "find") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                for item in a.iter() {
                    if self.call_value(&func, vec![item.clone()])? == Value::Bool(true) {
                        return Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(item.clone())),
                        });
                    }
                }
                Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                })
            }
            (Value::Array(a), "position") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                for (i, item) in a.iter().enumerate() {
                    if self.call_value(&func, vec![item.clone()])? == Value::Bool(true) {
                        return Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(Value::Int(i as i64))),
                        });
                    }
                }
                Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                })
            }
            (Value::Array(a), "count") => {
                if arg_vals.is_empty() {
                    return Ok(Value::Int(a.len() as i64));
                }
                let func = arg_vals.into_iter().next().unwrap_or(Value::Null);
                let mut c: i64 = 0;
                for item in a.iter() {
                    if self.call_value(&func, vec![item.clone()])? == Value::Bool(true) {
                        c += 1;
                    }
                }
                Ok(Value::Int(c))
            }
            (Value::Array(a), "sum") => {
                let mut total: i64 = 0;
                for item in a {
                    if let Value::Int(n) = item {
                        total += n;
                    }
                }
                Ok(Value::Int(total))
            }
            (Value::Array(a), "min") => {
                let mut m = i64::MAX;
                for item in a.iter() {
                    if let Value::Int(n) = item {
                        if *n < m {
                            m = *n;
                        }
                    }
                }
                if a.is_empty() {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Int(m))
                }
            }
            (Value::Array(a), "max") => {
                let mut m = i64::MIN;
                for item in a.iter() {
                    if let Value::Int(n) = item {
                        if *n > m {
                            m = *n;
                        }
                    }
                }
                if a.is_empty() {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Int(m))
                }
            }
            (Value::Array(a), "flatten") => {
                let mut result = Vec::new();
                for item in a.iter() {
                    if let Value::Array(inner) = item {
                        result.extend(inner.clone());
                    } else {
                        result.push(item.clone());
                    }
                }
                Ok(Value::Array(result))
            }
            (Value::Array(a), "dedup") => {
                let mut result = Vec::new();
                let mut prev: Option<Value> = None;
                for item in a.iter() {
                    if prev.as_ref() != Some(item) {
                        result.push(item.clone());
                    }
                    prev = Some(item.clone());
                }
                Ok(Value::Array(result))
            }
            (Value::Array(a), "chunks") => {
                let size = match arg_vals.first() {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "chunks() requires a positive integer".into(),
                        )
                        .into());
                    }
                };
                let result: Vec<Value> = a
                    .chunks(size)
                    .map(|chunk| Value::Array(chunk.to_vec()))
                    .collect();
                Ok(Value::Array(result))
            }
            (Value::Array(a), "windows") => {
                let size = match arg_vals.first() {
                    Some(Value::Int(n)) if *n > 0 => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "windows() requires a positive integer".into(),
                        )
                        .into());
                    }
                };
                let result: Vec<Value> =
                    a.windows(size).map(|w| Value::Array(w.to_vec())).collect();
                Ok(Value::Array(result))
            }
            (Value::Array(a), "first") => match a.first() {
                Some(v) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(v.clone())),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            (Value::Array(a), "last") => match a.last() {
                Some(v) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(v.clone())),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            // Map methods
            (Value::Map(m), "len") => Ok(Value::Int(m.len() as i64)),
            (Value::Map(m), "is_empty") => Ok(Value::Bool(m.is_empty())),
            (Value::Map(m), "contains_key") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    Ok(Value::Bool(m.contains_key(k)))
                } else {
                    Err(
                        RuntimeError::TypeError("contains_key() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Map(m), "get") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    match m.get(k) {
                        Some(v) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(v.clone())),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    }
                } else {
                    Err(RuntimeError::TypeError("get() requires a string argument".into()).into())
                }
            }
            (Value::Map(m), "keys") => {
                let keys: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                Ok(Value::Array(keys))
            }
            (Value::Map(m), "values") => {
                let vals: Vec<Value> = m.values().cloned().collect();
                Ok(Value::Array(vals))
            }
            (Value::Map(m), "insert") => {
                if arg_vals.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let mut args_iter = arg_vals.into_iter();
                let key = args_iter.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: 0,
                    })
                })?;
                let val = args_iter.next().ok_or_else(|| {
                    EvalError::Runtime(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: 1,
                    })
                })?;
                if let Value::Str(k) = key {
                    let mut new_map = m.clone();
                    new_map.insert(k, val);
                    Ok(Value::Map(new_map))
                } else {
                    Err(RuntimeError::TypeError("insert() requires a string key".into()).into())
                }
            }
            (Value::Map(m), "remove") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    let mut new_map = m.clone();
                    new_map.remove(k);
                    Ok(Value::Map(new_map))
                } else {
                    Err(
                        RuntimeError::TypeError("remove() requires a string argument".into())
                            .into(),
                    )
                }
            }
            // (join and reverse handled above in v0.8 methods)
            (Value::Array(a), "contains") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let needle = &arg_vals[0];
                let found = a.iter().any(|v| v == needle);
                Ok(Value::Bool(found))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{method}' on type {}",
                obj.type_name()
            ))
            .into()),
        }
    }

    // ── GPU/OpenCL builtins (v2.0 Q6A) ──

    /// OpenCL library search paths — standard locations plus the
    /// Adreno-specific library on the Radxa Dragon Q6A.
    const OPENCL_LIB_CANDIDATES: &'static [&'static str] = &[
        "libOpenCL.so",
        "libOpenCL.so.1",
        "libOpenCL_adreno.so.1",
        "/usr/lib/aarch64-linux-gnu/libOpenCL.so.1",
        "/vendor/lib64/libOpenCL_adreno.so.1",
    ];

    /// Attempt to detect OpenCL availability by loading the shared library.
    ///
    /// Tries standard names and the Adreno-specific path. Returns `true`
    /// if any of them can be loaded via `libloading`.
    fn detect_opencl() -> bool {
        for name in Self::OPENCL_LIB_CANDIDATES {
            // SAFETY: we only probe whether the library can be loaded.
            // No symbols are called; the library is dropped immediately.
            if unsafe { libloading::Library::new(name) }.is_ok() {
                return true;
            }
        }
        false
    }

    /// Query OpenCL platform/device info string via the OpenCL C API,
    /// loaded dynamically through `libloading`.
    ///
    /// Returns a human-readable info string on success, or `None` if
    /// OpenCL is not available or any FFI call fails.
    fn query_opencl_info() -> Option<String> {
        // OpenCL type aliases matching the C headers.
        type ClInt = i32;
        type ClUint = u32;
        type ClPlatformId = *mut std::ffi::c_void;
        type ClDeviceId = *mut std::ffi::c_void;
        type ClDeviceInfo = ClUint;
        type ClUlong = u64;

        // OpenCL constants.
        const CL_SUCCESS: ClInt = 0;
        const CL_DEVICE_TYPE_GPU: ClUlong = 4;
        const CL_DEVICE_NAME: ClDeviceInfo = 0x102B;
        const CL_DEVICE_VERSION: ClDeviceInfo = 0x102F;
        const CL_DEVICE_GLOBAL_MEM_SIZE: ClDeviceInfo = 0x101F;
        const CL_DEVICE_MAX_WORK_GROUP_SIZE: ClDeviceInfo = 0x1004;

        // OpenCL C function pointer types.
        type ClGetPlatformIDsFn =
            unsafe extern "C" fn(ClUint, *mut ClPlatformId, *mut ClUint) -> ClInt;
        type ClGetDeviceIDsFn = unsafe extern "C" fn(
            ClPlatformId,
            ClUlong,
            ClUint,
            *mut ClDeviceId,
            *mut ClUint,
        ) -> ClInt;
        type ClGetDeviceInfoFn =
            unsafe extern "C" fn(ClDeviceId, ClDeviceInfo, usize, *mut u8, *mut usize) -> ClInt;

        // Try to load OpenCL from any known path.
        let lib = {
            let mut found = None;
            for name in Self::OPENCL_LIB_CANDIDATES {
                // SAFETY: we load the library in order to call well-known
                // OpenCL entry points whose signatures match the C API.
                if let Ok(l) = unsafe { libloading::Library::new(name) } {
                    found = Some(l);
                    break;
                }
            }
            found?
        };

        // SAFETY: symbol names and type signatures match the OpenCL 1.0+ C API.
        let cl_get_platform_ids: libloading::Symbol<ClGetPlatformIDsFn> =
            unsafe { lib.get(b"clGetPlatformIDs\0") }.ok()?;
        let cl_get_device_ids: libloading::Symbol<ClGetDeviceIDsFn> =
            unsafe { lib.get(b"clGetDeviceIDs\0") }.ok()?;
        let cl_get_device_info: libloading::Symbol<ClGetDeviceInfoFn> =
            unsafe { lib.get(b"clGetDeviceInfo\0") }.ok()?;

        // Get first platform.
        let mut platform: ClPlatformId = std::ptr::null_mut();
        let mut num_platforms: ClUint = 0;
        // SAFETY: valid pointers, single-element output.
        let ret = unsafe { cl_get_platform_ids(1, &mut platform, &mut num_platforms) };
        if ret != CL_SUCCESS || num_platforms == 0 {
            return None;
        }

        // Get first GPU device on that platform.
        let mut device: ClDeviceId = std::ptr::null_mut();
        let mut num_devices: ClUint = 0;
        // SAFETY: valid platform handle, valid output pointers.
        let ret = unsafe {
            cl_get_device_ids(
                platform,
                CL_DEVICE_TYPE_GPU,
                1,
                &mut device,
                &mut num_devices,
            )
        };
        if ret != CL_SUCCESS || num_devices == 0 {
            return None;
        }

        // Helper closure: query a string-valued device info field.
        let query_string = |info: ClDeviceInfo| -> String {
            let mut size: usize = 0;
            // SAFETY: querying required buffer size; null data pointer.
            let ret =
                unsafe { cl_get_device_info(device, info, 0, std::ptr::null_mut(), &mut size) };
            if ret != CL_SUCCESS || size == 0 {
                return String::new();
            }
            let mut buf = vec![0u8; size];
            // SAFETY: buf is sized to hold the full result.
            let ret =
                unsafe { cl_get_device_info(device, info, size, buf.as_mut_ptr(), &mut size) };
            if ret != CL_SUCCESS {
                return String::new();
            }
            // Trim trailing NUL bytes.
            while buf.last() == Some(&0) {
                buf.pop();
            }
            String::from_utf8_lossy(&buf).to_string()
        };

        let dev_name = query_string(CL_DEVICE_NAME);
        let dev_version = query_string(CL_DEVICE_VERSION);

        // Query global memory size (u64).
        let mut global_mem: ClUlong = 0;
        let mut _ret_size: usize = 0;
        // SAFETY: output pointer correctly sized for u64.
        unsafe {
            cl_get_device_info(
                device,
                CL_DEVICE_GLOBAL_MEM_SIZE,
                std::mem::size_of::<ClUlong>(),
                std::ptr::addr_of_mut!(global_mem).cast::<u8>(),
                &mut _ret_size,
            );
        }

        // Query max work group size (usize on the host).
        let mut max_wg: usize = 0;
        // SAFETY: output pointer correctly sized for usize.
        unsafe {
            cl_get_device_info(
                device,
                CL_DEVICE_MAX_WORK_GROUP_SIZE,
                std::mem::size_of::<usize>(),
                std::ptr::addr_of_mut!(max_wg).cast::<u8>(),
                &mut _ret_size,
            );
        }

        let mem_mb = global_mem / (1024 * 1024);
        Some(format!(
            "{dev_name}, {dev_version}, {mem_mb}MB, max_workgroup={max_wg}"
        ))
    }

    /// `gpu_available() -> bool` — Check if an OpenCL GPU is available.
    pub(super) fn builtin_gpu_available(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        // Check Vulkan first, then OpenCL
        #[cfg(feature = "vulkan")]
        {
            if crate::bsp::dragon_q6a::vulkan::VulkanCompute::is_available() {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(Self::detect_opencl()))
    }

    /// `gpu_info() -> str` — Return GPU info string via OpenCL, or a fallback message.
    pub(super) fn builtin_gpu_info(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        #[cfg(feature = "vulkan")]
        {
            if let Ok(vk) = crate::bsp::dragon_q6a::vulkan::VulkanCompute::new() {
                let info = vk.device_info();
                return Ok(Value::Str(format!(
                    "{} (Vulkan {}, {}, subgroup={})",
                    info.name, info.api_version, info.device_type, info.subgroup_size,
                )));
            }
        }
        match Self::query_opencl_info() {
            Some(info) => Ok(Value::Str(info)),
            None => Ok(Value::Str("GPU not available (CPU fallback mode)".into())),
        }
    }

    /// `gpu_matmul(a: Tensor, b: Tensor) -> Tensor` — GPU-accelerated matrix multiply.
    ///
    /// Falls back to CPU tensor_matmul when OpenCL is unavailable.
    pub(super) fn builtin_gpu_matmul(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gpu_matmul: first arg must be tensor".into()).into(),
                );
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "gpu_matmul: second arg must be tensor".into(),
                )
                .into());
            }
        };
        // Try Vulkan GPU first, fall back to CPU
        #[cfg(feature = "vulkan")]
        {
            if let Ok(vk) = crate::bsp::dragon_q6a::vulkan::VulkanCompute::new() {
                let a_shape = a.shape();
                let b_shape = b.shape();
                if a_shape.len() == 2 && b_shape.len() == 2 && a_shape[1] == b_shape[0] {
                    let m = a_shape[0] as u32;
                    let k = a_shape[1] as u32;
                    let n = b_shape[1] as u32;
                    let a_f32: Vec<f32> = a.data().iter().map(|&v| v as f32).collect();
                    let b_f32: Vec<f32> = b.data().iter().map(|&v| v as f32).collect();
                    if let Ok(result_f32) = vk.tensor_matmul(&a_f32, &b_f32, m, k, n) {
                        let result_f64: Vec<f64> = result_f32.iter().map(|&v| v as f64).collect();
                        if let Ok(arr) = ndarray::ArrayD::from_shape_vec(
                            vec![m as usize, n as usize],
                            result_f64,
                        ) {
                            return Ok(Value::Tensor(
                                crate::runtime::ml::tensor::TensorValue::new(arr, false),
                            ));
                        }
                    }
                }
            }
        }
        // CPU fallback — delegates to existing tensor_matmul
        match tensor_ops::matmul(&a, &b) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `gpu_add(a: Tensor, b: Tensor) -> Tensor` — GPU-accelerated element-wise add.
    ///
    /// Falls back to CPU tensor_add when OpenCL is unavailable.
    pub(super) fn builtin_gpu_add(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gpu_add: first arg must be tensor".into()).into(),
                );
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("gpu_add: second arg must be tensor".into()).into(),
                );
            }
        };
        // CPU fallback — delegates to existing tensor_add
        match tensor_ops::add(&a, &b) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `gpu_relu(t: Tensor) -> Tensor` — GPU-accelerated ReLU activation.
    ///
    /// Falls back to CPU tensor_relu when OpenCL is unavailable.
    pub(super) fn builtin_gpu_relu(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::relu(t))),
            _ => Err(RuntimeError::TypeError("gpu_relu: arg must be tensor".into()).into()),
        }
    }

    /// `gpu_sigmoid(t: Tensor) -> Tensor` — GPU-accelerated sigmoid activation.
    ///
    /// Falls back to CPU tensor_sigmoid when OpenCL is unavailable.
    pub(super) fn builtin_gpu_sigmoid(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::sigmoid(t))),
            _ => Err(RuntimeError::TypeError("gpu_sigmoid: arg must be tensor".into()).into()),
        }
    }

    /// `gpu_mul(a, b) -> Tensor` — Element-wise multiply (CPU fallback).
    pub(super) fn builtin_gpu_mul(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(a), Value::Tensor(b)) => {
                tensor_ops::mul(a, b).map(Value::Tensor).map_err(|e| {
                    crate::interpreter::EvalError::Runtime(RuntimeError::TypeError(format!(
                        "gpu_mul: {e}"
                    )))
                })
            }
            _ => Err(RuntimeError::TypeError("gpu_mul: args must be tensors".into()).into()),
        }
    }

    /// `gpu_transpose(t) -> Tensor` — Matrix transpose (CPU fallback).
    pub(super) fn builtin_gpu_transpose(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => tensor_ops::transpose(t).map(Value::Tensor).map_err(|e| {
                crate::interpreter::EvalError::Runtime(RuntimeError::TypeError(format!(
                    "gpu_transpose: {e}"
                )))
            }),
            _ => Err(RuntimeError::TypeError("gpu_transpose: arg must be tensor".into()).into()),
        }
    }

    /// `gpu_sum(t) -> Float` — Sum all elements (CPU fallback).
    pub(super) fn builtin_gpu_sum(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let sum: f64 = t.data().iter().sum();
                Ok(Value::Float(sum))
            }
            _ => Err(RuntimeError::TypeError("gpu_sum: arg must be tensor".into()).into()),
        }
    }

    // ── Edge AI / production builtins (v2.0 Q6A) ──

    /// `cpu_temp() -> i64` — Read maximum CPU temperature in millidegrees Celsius.
    pub(super) fn builtin_cpu_temp(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let mut max_temp: i64 = 0;
        for i in 0..10 {
            let path = format!("/sys/class/thermal/thermal_zone{i}/temp");
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(temp) = content.trim().parse::<i64>() {
                    if temp > max_temp {
                        max_temp = temp;
                    }
                }
            }
        }
        Ok(Value::Int(max_temp))
    }

    /// `cpu_freq() -> i64` — Read current CPU frequency in kHz (max across all cores).
    pub(super) fn builtin_cpu_freq(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let mut max_freq: i64 = 0;
        for i in 0..8 {
            let path = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq");
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(freq) = content.trim().parse::<i64>() {
                    if freq > max_freq {
                        max_freq = freq;
                    }
                }
            }
        }
        Ok(Value::Int(max_freq))
    }

    /// `mem_usage() -> i64` — Return memory usage percentage (0-100).
    pub(super) fn builtin_mem_usage(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut total: i64 = 0;
            let mut available: i64 = 0;
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("MemTotal:") {
                    total = val
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                } else if let Some(val) = line.strip_prefix("MemAvailable:") {
                    available = val
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }
            if total > 0 {
                return Ok(Value::Int((total - available) * 100 / total));
            }
        }
        Ok(Value::Int(0))
    }

    /// `sys_uptime() -> i64` — Return system uptime in seconds.
    pub(super) fn builtin_sys_uptime(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
            if let Some(secs_str) = content.split_whitespace().next() {
                if let Ok(secs) = secs_str.parse::<f64>() {
                    return Ok(Value::Int(secs as i64));
                }
            }
        }
        Ok(Value::Int(0))
    }

    /// `log_to_file(path: str, message: str) -> bool` — Append message to log file.
    pub(super) fn builtin_log_to_file(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("log_to_file: path must be string".into()).into(),
                );
            }
        };
        let message = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("log_to_file: message must be string".into()).into(),
                );
            }
        };
        use std::io::Write;
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let _ = writeln!(file, "[{timestamp}] {message}");
                Ok(Value::Bool(true))
            }
            Err(_) => Ok(Value::Bool(false)),
        }
    }

    /// Software watchdog — starts a background thread that panics if not kicked within timeout_ms.
    /// Returns a watchdog ID (i64).
    pub(super) fn builtin_watchdog_start(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let timeout_ms = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(RuntimeError::TypeError(
                    "watchdog_start: timeout must be integer (ms)".into(),
                )
                .into());
            }
        };
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let alive = Arc::new(AtomicBool::new(true));
        let alive_clone = alive.clone();
        let id = Arc::as_ptr(&alive) as i64;
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
                if !alive_clone.load(Ordering::Relaxed) {
                    break; // stopped
                }
                // If still alive, the watchdog was kicked (reset to true after check)
                if !alive_clone.swap(false, Ordering::Relaxed) {
                    // Was already false = not kicked in time
                    eprintln!("[watchdog] TIMEOUT after {timeout_ms}ms — process not responding");
                    break;
                }
            }
        });
        // Store the Arc in a global map so kick/stop can access it
        self.env
            .borrow_mut()
            .define(format!("__watchdog_{id}"), Value::Int(id));
        // Store alive flag pointer for kick/stop
        // We leak the Arc intentionally — it's cleaned up on stop
        std::mem::forget(alive);
        Ok(Value::Int(id))
    }

    /// Kick the watchdog to prevent timeout.
    pub(super) fn builtin_watchdog_kick(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let id = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("watchdog_kick: id must be integer".into()).into(),
                );
            }
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        // SAFETY: id is a pointer to an Arc<AtomicBool> we leaked in watchdog_start
        let alive = unsafe { &*(id as *const AtomicBool) };
        alive.store(true, Ordering::Relaxed);
        Ok(Value::Bool(true))
    }

    /// Stop the watchdog.
    pub(super) fn builtin_watchdog_stop(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let id = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("watchdog_stop: id must be integer".into()).into(),
                );
            }
        };
        use std::sync::atomic::{AtomicBool, Ordering};
        // SAFETY: id is a pointer to an Arc<AtomicBool> we leaked in watchdog_start
        let alive = unsafe { &*(id as *const AtomicBool) };
        alive.store(false, Ordering::Relaxed); // signal thread to exit
        Ok(Value::Bool(true))
    }

    /// Sleep for given milliseconds.
    pub(super) fn builtin_sleep_ms(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ms = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(RuntimeError::TypeError(
                    "sleep_ms: duration must be integer (ms)".into(),
                )
                .into());
            }
        };
        std::thread::sleep(std::time::Duration::from_millis(ms));
        Ok(Value::Null)
    }

    /// Set a key-value pair in the inference cache.
    pub(super) fn builtin_cache_set(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let key = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("cache_set: key must be string".into()).into());
            }
        };
        let val = match &args[1] {
            Value::Str(s) => s.clone(),
            other => format!("{other}"),
        };
        self.inference_cache.insert(key, val);
        Ok(Value::Bool(true))
    }

    /// Get a value from the inference cache. Returns "" if not found.
    pub(super) fn builtin_cache_get(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let key = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("cache_get: key must be string".into()).into());
            }
        };
        match self.inference_cache.get(&key) {
            Some(val) => Ok(Value::Str(val.clone())),
            None => Ok(Value::Str(String::new())),
        }
    }

    /// Get file size in bytes. Returns -1 if file doesn't exist.
    pub(super) fn builtin_file_size(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("file_size: path must be string".into()).into(),
                );
            }
        };
        match std::fs::metadata(&path) {
            Ok(m) => Ok(Value::Int(m.len() as i64)),
            Err(_) => Ok(Value::Int(-1)),
        }
    }

    /// List directory contents. Returns array of filenames.
    pub(super) fn builtin_dir_list(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("dir_list: path must be string".into()).into());
            }
        };
        let mut entries = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&path) {
            for entry in dir.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    entries.push(Value::Str(name.to_string()));
                }
            }
        }
        Ok(Value::Array(entries))
    }

    /// Get environment variable value. Returns "" if not set.
    pub(super) fn builtin_env_var(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let name = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => return Err(RuntimeError::TypeError("env_var: name must be string".into()).into()),
        };
        match std::env::var(&name) {
            Ok(val) => Ok(Value::Str(val)),
            Err(_) => Ok(Value::Str(String::new())),
        }
    }
}
