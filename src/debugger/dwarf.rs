//! DWARF debug information generation for Fajar Lang.
//!
//! Collects source mapping data during codegen and generates DWARF sections
//! for use with native debuggers (GDB, LLDB).
//!
//! # Architecture
//!
//! ```text
//! Codegen (Cranelift / LLVM)
//!     │ emit_source_loc(offset, line)
//!     ▼
//! SourceMap (offset → line pairs)
//!     │
//!     ▼
//! DwarfGenerator
//!     ├── DW_TAG_compile_unit (file, producer)
//!     ├── DW_TAG_subprogram (function name, low_pc, high_pc)
//!     ├── DW_TAG_variable (name, type, location)
//!     ├── DW_TAG_base_type (i64, f64, bool, str)
//!     └── .debug_line (line number program)
//! ```

use std::collections::HashMap;

/// A mapping from instruction offset to source line number.
#[derive(Debug, Clone)]
pub struct SourceMapping {
    /// Instruction byte offset within the function.
    pub offset: u64,
    /// 1-based source line number.
    pub line: u32,
    /// 1-based source column number.
    pub column: u32,
}

/// Collects source location mappings during code generation.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    /// Source file name.
    pub file: String,
    /// Source directory.
    pub directory: String,
    /// Instruction-to-line mappings for each function.
    functions: HashMap<String, FunctionDebugInfo>,
    /// Global line mappings (for non-function code).
    line_mappings: Vec<SourceMapping>,
}

/// Debug information for a single function.
#[derive(Debug, Clone)]
pub struct FunctionDebugInfo {
    /// Function name.
    pub name: String,
    /// Start offset (low_pc).
    pub start_offset: u64,
    /// End offset (high_pc).
    pub end_offset: u64,
    /// Source line where the function is defined.
    pub source_line: u32,
    /// Source-to-instruction mappings within this function.
    pub line_mappings: Vec<SourceMapping>,
    /// Local variables with their debug info.
    pub variables: Vec<VariableDebugInfo>,
}

/// Debug information for a local variable.
#[derive(Debug, Clone)]
pub struct VariableDebugInfo {
    /// Variable name.
    pub name: String,
    /// Type name (e.g., "i64", "f64", "bool").
    pub type_name: String,
    /// Stack frame offset (for DW_OP_fbreg).
    pub frame_offset: i64,
    /// Source line where variable is declared.
    pub decl_line: u32,
}

/// DWARF base type encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwarfBaseType {
    /// Signed integer (DW_ATE_signed).
    SignedInt { byte_size: u8 },
    /// Unsigned integer (DW_ATE_unsigned).
    UnsignedInt { byte_size: u8 },
    /// Float (DW_ATE_float).
    Float { byte_size: u8 },
    /// Boolean (DW_ATE_boolean).
    Boolean,
    /// UTF-8 string (pointer + length struct).
    Utf8String,
}

impl DwarfBaseType {
    /// Maps a Fajar Lang type name to a DWARF base type.
    pub fn from_fajar_type(type_name: &str) -> Option<Self> {
        match type_name {
            "bool" => Some(DwarfBaseType::Boolean),
            "i8" => Some(DwarfBaseType::SignedInt { byte_size: 1 }),
            "i16" => Some(DwarfBaseType::SignedInt { byte_size: 2 }),
            "i32" => Some(DwarfBaseType::SignedInt { byte_size: 4 }),
            "i64" | "isize" => Some(DwarfBaseType::SignedInt { byte_size: 8 }),
            "i128" => Some(DwarfBaseType::SignedInt { byte_size: 16 }),
            "u8" => Some(DwarfBaseType::UnsignedInt { byte_size: 1 }),
            "u16" => Some(DwarfBaseType::UnsignedInt { byte_size: 2 }),
            "u32" => Some(DwarfBaseType::UnsignedInt { byte_size: 4 }),
            "u64" | "usize" => Some(DwarfBaseType::UnsignedInt { byte_size: 8 }),
            "u128" => Some(DwarfBaseType::UnsignedInt { byte_size: 16 }),
            "f32" => Some(DwarfBaseType::Float { byte_size: 4 }),
            "f64" => Some(DwarfBaseType::Float { byte_size: 8 }),
            "str" => Some(DwarfBaseType::Utf8String),
            _ => None,
        }
    }

    /// Returns the DW_ATE encoding value.
    pub fn encoding(&self) -> u8 {
        match self {
            DwarfBaseType::SignedInt { .. } => 0x05,   // DW_ATE_signed
            DwarfBaseType::UnsignedInt { .. } => 0x07, // DW_ATE_unsigned
            DwarfBaseType::Float { .. } => 0x04,       // DW_ATE_float
            DwarfBaseType::Boolean => 0x02,            // DW_ATE_boolean
            DwarfBaseType::Utf8String => 0x08,         // DW_ATE_unsigned_char
        }
    }

    /// Returns the byte size of the type.
    pub fn byte_size(&self) -> u8 {
        match self {
            DwarfBaseType::SignedInt { byte_size } => *byte_size,
            DwarfBaseType::UnsignedInt { byte_size } => *byte_size,
            DwarfBaseType::Float { byte_size } => *byte_size,
            DwarfBaseType::Boolean => 1,
            DwarfBaseType::Utf8String => 16, // ptr + len
        }
    }
}

impl SourceMap {
    /// Creates a new source map for a given file.
    pub fn new(file: &str, directory: &str) -> Self {
        Self {
            file: file.to_string(),
            directory: directory.to_string(),
            functions: HashMap::new(),
            line_mappings: Vec::new(),
        }
    }

    /// Begins tracking a new function.
    pub fn begin_function(&mut self, name: &str, start_offset: u64, source_line: u32) {
        self.functions.insert(
            name.to_string(),
            FunctionDebugInfo {
                name: name.to_string(),
                start_offset,
                end_offset: start_offset,
                source_line,
                line_mappings: Vec::new(),
                variables: Vec::new(),
            },
        );
    }

    /// Sets the end offset for a function.
    pub fn end_function(&mut self, name: &str, end_offset: u64) {
        if let Some(func) = self.functions.get_mut(name) {
            func.end_offset = end_offset;
        }
    }

    /// Adds a source location mapping within a function.
    pub fn add_mapping(&mut self, func_name: &str, offset: u64, line: u32, column: u32) {
        let mapping = SourceMapping {
            offset,
            line,
            column,
        };
        if let Some(func) = self.functions.get_mut(func_name) {
            func.line_mappings.push(mapping);
        } else {
            self.line_mappings.push(mapping);
        }
    }

    /// Adds a variable debug entry for a function.
    pub fn add_variable(
        &mut self,
        func_name: &str,
        name: &str,
        type_name: &str,
        frame_offset: i64,
        decl_line: u32,
    ) {
        if let Some(func) = self.functions.get_mut(func_name) {
            func.variables.push(VariableDebugInfo {
                name: name.to_string(),
                type_name: type_name.to_string(),
                frame_offset,
                decl_line,
            });
        }
    }

    /// Returns all function debug info entries.
    pub fn functions(&self) -> &HashMap<String, FunctionDebugInfo> {
        &self.functions
    }

    /// Returns the total number of source mappings across all functions.
    pub fn total_mappings(&self) -> usize {
        self.line_mappings.len()
            + self
                .functions
                .values()
                .map(|f| f.line_mappings.len())
                .sum::<usize>()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_basic_usage() {
        let mut sm = SourceMap::new("test.fj", "/home/user/project");
        assert_eq!(sm.file, "test.fj");
        assert_eq!(sm.total_mappings(), 0);

        sm.begin_function("main", 0, 1);
        sm.add_mapping("main", 0, 1, 1);
        sm.add_mapping("main", 8, 2, 5);
        sm.add_mapping("main", 16, 3, 1);
        sm.end_function("main", 24);

        assert_eq!(sm.total_mappings(), 3);
        let funcs = sm.functions();
        assert!(funcs.contains_key("main"));
        let main = &funcs["main"];
        assert_eq!(main.start_offset, 0);
        assert_eq!(main.end_offset, 24);
        assert_eq!(main.source_line, 1);
        assert_eq!(main.line_mappings.len(), 3);
    }

    #[test]
    fn source_map_variables() {
        let mut sm = SourceMap::new("test.fj", ".");
        sm.begin_function("foo", 0, 5);
        sm.add_variable("foo", "x", "i64", -8, 6);
        sm.add_variable("foo", "y", "f64", -16, 7);

        let foo = &sm.functions()["foo"];
        assert_eq!(foo.variables.len(), 2);
        assert_eq!(foo.variables[0].name, "x");
        assert_eq!(foo.variables[0].frame_offset, -8);
        assert_eq!(foo.variables[1].name, "y");
        assert_eq!(foo.variables[1].type_name, "f64");
    }

    #[test]
    fn dwarf_base_type_mapping() {
        assert_eq!(
            DwarfBaseType::from_fajar_type("i64"),
            Some(DwarfBaseType::SignedInt { byte_size: 8 })
        );
        assert_eq!(
            DwarfBaseType::from_fajar_type("f32"),
            Some(DwarfBaseType::Float { byte_size: 4 })
        );
        assert_eq!(
            DwarfBaseType::from_fajar_type("bool"),
            Some(DwarfBaseType::Boolean)
        );
        assert_eq!(
            DwarfBaseType::from_fajar_type("str"),
            Some(DwarfBaseType::Utf8String)
        );
        assert!(DwarfBaseType::from_fajar_type("SomeStruct").is_none());
    }

    #[test]
    fn dwarf_base_type_encoding() {
        assert_eq!(DwarfBaseType::SignedInt { byte_size: 8 }.encoding(), 0x05);
        assert_eq!(DwarfBaseType::UnsignedInt { byte_size: 4 }.encoding(), 0x07);
        assert_eq!(DwarfBaseType::Float { byte_size: 4 }.encoding(), 0x04);
        assert_eq!(DwarfBaseType::Boolean.encoding(), 0x02);
    }

    #[test]
    fn dwarf_base_type_byte_size() {
        assert_eq!(DwarfBaseType::SignedInt { byte_size: 8 }.byte_size(), 8);
        assert_eq!(DwarfBaseType::Boolean.byte_size(), 1);
        assert_eq!(DwarfBaseType::Utf8String.byte_size(), 16);
        assert_eq!(DwarfBaseType::Float { byte_size: 4 }.byte_size(), 4);
    }

    #[test]
    fn source_map_multiple_functions() {
        let mut sm = SourceMap::new("multi.fj", ".");
        sm.begin_function("add", 0, 1);
        sm.add_mapping("add", 0, 1, 1);
        sm.end_function("add", 16);

        sm.begin_function("main", 16, 5);
        sm.add_mapping("main", 16, 5, 1);
        sm.add_mapping("main", 24, 6, 5);
        sm.end_function("main", 32);

        assert_eq!(sm.functions().len(), 2);
        assert_eq!(sm.total_mappings(), 3);
    }

    #[test]
    fn function_debug_info_line_entries() {
        let mut sm = SourceMap::new("test.fj", ".");
        sm.begin_function("calc", 100, 10);
        sm.add_mapping("calc", 100, 10, 1);
        sm.add_mapping("calc", 108, 11, 5);
        sm.add_mapping("calc", 112, 12, 5);
        sm.add_mapping("calc", 120, 13, 1);
        sm.end_function("calc", 128);

        let calc = &sm.functions()["calc"];
        assert_eq!(calc.line_mappings.len(), 4);
        assert_eq!(calc.line_mappings[0].line, 10);
        assert_eq!(calc.line_mappings[3].offset, 120);
    }

    #[test]
    fn all_integer_types_map_correctly() {
        for (ty, expected_size) in [
            ("i8", 1),
            ("i16", 2),
            ("i32", 4),
            ("i64", 8),
            ("i128", 16),
            ("u8", 1),
            ("u16", 2),
            ("u32", 4),
            ("u64", 8),
            ("u128", 16),
        ] {
            let dt = DwarfBaseType::from_fajar_type(ty).unwrap();
            assert_eq!(dt.byte_size(), expected_size, "failed for type {ty}");
        }
    }
}
