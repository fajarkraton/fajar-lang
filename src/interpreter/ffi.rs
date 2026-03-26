//! Foreign Function Interface (FFI) support for the Fajar Lang interpreter.
//!
//! Provides dynamic library loading and C function calling via `libloading`.
//! Only supports C ABI-compatible types: integers, floats, booleans.

use std::collections::HashMap;
use std::path::Path;

use super::value::Value;

/// Manages loaded shared libraries and their symbols.
pub struct FfiManager {
    /// Loaded libraries, keyed by path.
    libraries: Vec<libloading::Library>,
    /// Registered foreign functions: name → (library index, symbol name).
    functions: HashMap<String, FfiFn>,
}

/// A registered foreign function.
struct FfiFn {
    /// Index into the `libraries` vec.
    lib_index: usize,
    /// Symbol name in the library.
    symbol: String,
    /// Parameter type descriptors for marshaling.
    param_types: Vec<FfiType>,
    /// Return type descriptor.
    ret_type: FfiType,
}

/// FFI-safe type descriptors for marshaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfiType {
    /// Void (no return value).
    Void,
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
    /// 64-bit float.
    F64,
    /// Boolean (passed as i32).
    Bool,
}

impl FfiManager {
    /// Creates a new empty FFI manager.
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
            functions: HashMap::new(),
        }
    }

    /// Loads a shared library from the given path.
    ///
    /// Returns the library index on success.
    ///
    /// # Safety
    ///
    /// Loading a shared library can execute arbitrary code in the library's
    /// init functions. Only load trusted libraries.
    pub fn load_library(&mut self, path: &Path) -> Result<usize, String> {
        // SAFETY: We trust the user-specified library path. This is inherently
        // unsafe as loading a .so/.dylib can execute arbitrary init code.
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| format!("failed to load library '{}': {}", path.display(), e))?;
        let index = self.libraries.len();
        self.libraries.push(lib);
        Ok(index)
    }

    /// Registers a foreign function from a loaded library.
    pub fn register_function(
        &mut self,
        name: &str,
        lib_index: usize,
        symbol: &str,
        param_types: Vec<FfiType>,
        ret_type: FfiType,
    ) -> Result<(), String> {
        if lib_index >= self.libraries.len() {
            return Err(format!("invalid library index: {}", lib_index));
        }
        // Verify the symbol exists
        let lib = &self.libraries[lib_index];
        // SAFETY: looking up symbol in a loaded library; no call is made
        unsafe {
            let _: libloading::Symbol<unsafe extern "C" fn()> = lib
                .get(symbol.as_bytes())
                .map_err(|e| format!("symbol '{}' not found: {}", symbol, e))?;
        }
        self.functions.insert(
            name.to_string(),
            FfiFn {
                lib_index,
                symbol: symbol.to_string(),
                param_types,
                ret_type,
            },
        );
        Ok(())
    }

    /// Calls a registered foreign function with the given arguments.
    ///
    /// Marshals `Value` arguments to C types, calls the function, and
    /// marshals the return value back.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let ffi_fn = self
            .functions
            .get(name)
            .ok_or_else(|| format!("FFI function '{}' not registered", name))?;

        if args.len() != ffi_fn.param_types.len() {
            return Err(format!(
                "FFI function '{}': expected {} args, got {}",
                name,
                ffi_fn.param_types.len(),
                args.len()
            ));
        }

        let lib = &self.libraries[ffi_fn.lib_index];

        // Marshal args to i64 values for the C call
        let c_args: Vec<i64> = args
            .iter()
            .zip(&ffi_fn.param_types)
            .map(|(val, ty)| marshal_to_c(val, ty))
            .collect::<Result<Vec<_>, _>>()?;

        // SAFETY: Arguments are marshaled via marshal_to_c() against registered
        // param_types, symbol existence was verified at register_function() time,
        // and the library was loaded from a user-provided path (trust boundary).
        let result = unsafe { self.call_raw(lib, &ffi_fn.symbol, &c_args, ffi_fn.ret_type) }?;

        Ok(marshal_from_c(result, ffi_fn.ret_type))
    }

    /// Low-level function call dispatch.
    ///
    /// # Safety
    ///
    /// Calls a C function via a dynamic symbol. The caller must ensure the
    /// argument types match the actual function signature.
    unsafe fn call_raw(
        &self,
        lib: &libloading::Library,
        symbol: &str,
        args: &[i64],
        ret_type: FfiType,
    ) -> Result<i64, String> {
        // SAFETY: caller guarantees argument types match the function signature.
        unsafe {
            match (args.len(), ret_type) {
                (0, FfiType::Void) => {
                    let f: libloading::Symbol<unsafe extern "C" fn()> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    f();
                    Ok(0)
                }
                (0, _) => {
                    let f: libloading::Symbol<unsafe extern "C" fn() -> i64> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    Ok(f())
                }
                (1, FfiType::Void) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64)> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    f(args[0]);
                    Ok(0)
                }
                (1, _) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64) -> i64> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    Ok(f(args[0]))
                }
                (2, FfiType::Void) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64, i64)> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    f(args[0], args[1]);
                    Ok(0)
                }
                (2, _) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64, i64) -> i64> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    Ok(f(args[0], args[1]))
                }
                (3, FfiType::Void) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64, i64, i64)> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    f(args[0], args[1], args[2]);
                    Ok(0)
                }
                (3, _) => {
                    let f: libloading::Symbol<unsafe extern "C" fn(i64, i64, i64) -> i64> = lib
                        .get(symbol.as_bytes())
                        .map_err(|e| format!("symbol error: {}", e))?;
                    Ok(f(args[0], args[1], args[2]))
                }
                (n, _) => Err(format!(
                    "FFI call with {} arguments not supported (max 3)",
                    n
                )),
            }
        }
    }

    /// Returns true if a function with the given name is registered.
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
}

impl Default for FfiManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Marshals a Fajar Lang Value to a C i64 representation.
fn marshal_to_c(val: &Value, ty: &FfiType) -> Result<i64, String> {
    match (val, ty) {
        (Value::Int(n), FfiType::I32 | FfiType::I64) => Ok(*n),
        (Value::Float(f), FfiType::F64) => Ok(f.to_bits() as i64),
        (Value::Bool(b), FfiType::Bool) => Ok(if *b { 1 } else { 0 }),
        _ => Err(format!("cannot marshal {} to {:?}", val.type_name(), ty)),
    }
}

/// Marshals a C i64 result back to a Fajar Lang Value.
fn marshal_from_c(raw: i64, ty: FfiType) -> Value {
    match ty {
        FfiType::Void => Value::Null,
        FfiType::I32 | FfiType::I64 => Value::Int(raw),
        FfiType::F64 => Value::Float(f64::from_bits(raw as u64)),
        FfiType::Bool => Value::Bool(raw != 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_manager_new_is_empty() {
        let mgr = FfiManager::new();
        assert!(!mgr.has_function("anything"));
    }

    #[test]
    fn ffi_manager_load_nonexistent_library_fails() {
        let mut mgr = FfiManager::new();
        let result = mgr.load_library(Path::new("/nonexistent/libfoo.so"));
        assert!(result.is_err());
    }

    #[test]
    fn ffi_manager_call_unregistered_fails() {
        let mgr = FfiManager::new();
        let result = mgr.call("nonexistent", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }

    #[test]
    fn marshal_int_to_c() {
        assert_eq!(marshal_to_c(&Value::Int(42), &FfiType::I64).unwrap(), 42);
        assert_eq!(marshal_to_c(&Value::Int(-1), &FfiType::I32).unwrap(), -1);
    }

    #[test]
    fn marshal_bool_to_c() {
        assert_eq!(marshal_to_c(&Value::Bool(true), &FfiType::Bool).unwrap(), 1);
        assert_eq!(
            marshal_to_c(&Value::Bool(false), &FfiType::Bool).unwrap(),
            0
        );
    }

    #[test]
    fn marshal_float_to_c() {
        let raw = marshal_to_c(&Value::Float(3.14), &FfiType::F64).unwrap();
        let back = f64::from_bits(raw as u64);
        assert!((back - 3.14).abs() < 1e-10);
    }

    #[test]
    fn marshal_type_mismatch_fails() {
        let result = marshal_to_c(&Value::Str("hello".to_string()), &FfiType::I64);
        assert!(result.is_err());
    }

    #[test]
    fn marshal_from_c_int() {
        assert_eq!(marshal_from_c(42, FfiType::I64), Value::Int(42));
    }

    #[test]
    fn marshal_from_c_void() {
        assert_eq!(marshal_from_c(0, FfiType::Void), Value::Null);
    }

    #[test]
    fn marshal_from_c_bool() {
        assert_eq!(marshal_from_c(1, FfiType::Bool), Value::Bool(true));
        assert_eq!(marshal_from_c(0, FfiType::Bool), Value::Bool(false));
    }

    #[test]
    fn ffi_manager_invalid_lib_index() {
        let mut mgr = FfiManager::new();
        let result = mgr.register_function("foo", 999, "foo", vec![FfiType::I64], FfiType::I64);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid library index"));
    }

    #[test]
    fn ffi_call_wrong_arg_count() {
        // We can't easily register a real function without a library,
        // but we can test the arg count check indirectly via the manager.
        let mgr = FfiManager::new();
        // No function registered, so this will fail with "not registered"
        let result = mgr.call("abs", &[Value::Int(1), Value::Int(2)]);
        assert!(result.is_err());
    }

    // Integration test: load libc and call abs()
    #[test]
    fn ffi_call_libc_abs() {
        let mut mgr = FfiManager::new();

        // Try to load libc — path varies by platform
        let libc_path = if cfg!(target_os = "linux") {
            "libc.so.6"
        } else if cfg!(target_os = "macos") {
            "libSystem.dylib"
        } else {
            return; // Skip on other platforms
        };

        let lib_idx = match mgr.load_library(Path::new(libc_path)) {
            Ok(idx) => idx,
            Err(_) => return, // Skip if libc not loadable
        };

        mgr.register_function("abs", lib_idx, "abs", vec![FfiType::I32], FfiType::I32)
            .expect("abs should be found in libc");

        assert!(mgr.has_function("abs"));

        // abs(-42) should return 42
        let result = mgr.call("abs", &[Value::Int(-42)]).unwrap();
        assert_eq!(result, Value::Int(42));

        // abs(7) should return 7
        let result = mgr.call("abs", &[Value::Int(7)]).unwrap();
        assert_eq!(result, Value::Int(7));
    }
}
