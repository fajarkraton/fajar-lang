//! Shared codegen context and cleanup utilities.
//!
//! Contains `CodegenCtx` (the shared state bundle passed to all compilation functions)
//! and `OwnedKind` (resource tracking for heap cleanup).

use std::collections::{HashMap, HashSet};

use cranelift_codegen::ir::{InstBuilder, StackSlot, Value as ClifValue};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataId, FuncId, Module};

use super::super::CodegenError;

/// Kind of heap-allocated resource that needs cleanup at scope exit.
#[derive(Debug, Clone)]
pub(crate) enum OwnedKind {
    /// Heap-allocated string (fj_rt_str_concat result). Free with fj_rt_free(ptr, len).
    String,
    /// Heap-allocated dynamic array (fj_rt_array_new result). Free with fj_rt_array_free(ptr).
    Array,
    /// Heap-allocated HashMap (fj_rt_map_new result). Free with fj_rt_map_free(ptr).
    Map,
    /// BumpAllocator handle. Destroy with fj_rt_bump_destroy(ptr).
    BumpAllocator,
    /// FreeListAllocator handle. Destroy with fj_rt_freelist_destroy(ptr).
    FreeListAllocator,
    /// PoolAllocator handle. Destroy with fj_rt_pool_destroy(ptr).
    PoolAllocator,
}

/// Bundles shared codegen state passed to all free-standing compile functions.
pub(crate) struct CodegenCtx<'a, M: Module> {
    pub module: &'a mut M,
    pub functions: &'a HashMap<String, FuncId>,
    pub var_map: &'a mut HashMap<String, Variable>,
    pub string_data: &'a mut HashMap<String, DataId>,
    /// Monomorphization map: generic fn name → mangled specialized name.
    pub mono_map: &'a HashMap<String, String>,
    /// Array metadata: variable name → (stack_slot, length).
    pub array_meta: &'a mut HashMap<String, (StackSlot, usize)>,
    /// Temporarily holds metadata from the last compiled array literal.
    pub last_array: Option<(StackSlot, usize)>,
    /// Current loop's exit block (for `break`).
    pub loop_exit: Option<cranelift_codegen::ir::Block>,
    /// Current loop's header block (for `continue`).
    pub loop_header: Option<cranelift_codegen::ir::Block>,
    /// Tracks the Cranelift type of each variable for type-aware codegen.
    pub var_types: &'a mut HashMap<String, cranelift_codegen::ir::Type>,
    /// Tracks the return type of each function for type-aware dispatch.
    pub fn_return_types: &'a HashMap<String, cranelift_codegen::ir::Type>,
    /// Type of the most recently compiled expression (for Let type inference).
    pub last_expr_type: Option<cranelift_codegen::ir::Type>,
    /// For string variables: maps var name → Variable holding the string length.
    pub string_lens: &'a mut HashMap<String, Variable>,
    /// Holds the length ClifValue of the last compiled string expression.
    pub last_string_len: Option<ClifValue>,
    /// True when the last string expression was heap-allocated (needs free).
    pub last_string_owned: bool,
    /// Names of variables that are heap-allocated dynamic arrays (Vec-backed).
    pub heap_arrays: &'a mut HashSet<String>,
    /// Names of variables that hold heap-allocated HashMaps.
    pub heap_maps: HashSet<String>,
    /// Names of map variables that store string values (inserted via map_insert_str).
    pub map_str_values: HashSet<String>,
    /// True when the last expression produced a HashMap (fj_rt_map_new).
    pub last_map_new: bool,
    /// Enum definitions: enum name → list of variant names (index = tag).
    pub enum_defs: &'a HashMap<String, Vec<String>>,
    /// Tracks variables that hold enum values: name → (tag_var, payload_var, payload_type).
    pub enum_vars: &'a mut HashMap<String, (Variable, Variable, cranelift_codegen::ir::Type)>,
    /// Payload of the last compiled enum constructor expression.
    pub last_enum_payload: Option<ClifValue>,
    /// Type of the last enum payload (for type-aware destructuring).
    pub last_enum_payload_type: Option<cranelift_codegen::ir::Type>,
    /// Struct definitions: struct name → ordered list of (field_name, clif_type).
    pub struct_defs: &'a HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    /// Set of type names that are unions (all fields at offset 0).
    pub union_names: &'a HashSet<String>,
    /// Bitfield layouts: struct_name → vec of (field_name, bit_offset, bit_width).
    /// Structs with u1-u7 fields pack those into a single i64 word.
    pub bitfield_layouts: &'a HashMap<String, Vec<(String, u8, u8)>>,
    /// Tracks struct variables: var name → (stack_slot, struct_type_name).
    pub struct_slots: &'a mut HashMap<String, (StackSlot, String)>,
    /// Side-channel: set by compile_struct_init with (slot, struct_name).
    pub last_struct_init: Option<(StackSlot, String)>,
    /// Tuple element types: var_name → Vec<ClifType> for type-aware tuple index access.
    pub tuple_types: HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    /// Side-channel: element types from the last compiled tuple expression.
    pub last_tuple_elem_types: Option<Vec<cranelift_codegen::ir::Type>>,
    /// Impl methods: (type_name, method_name) → mangled function name.
    pub impl_methods: &'a HashMap<(String, String), String>,
    /// Trait definitions: trait name → list of required method names.
    pub trait_defs: &'a HashMap<String, Vec<String>>,
    /// Trait impls: (trait_name, type_name) → list of method names implemented.
    #[allow(dead_code)]
    pub trait_impls: &'a HashMap<(String, String), Vec<String>>,
    /// Tracks heap-allocated resources that need cleanup at function exit.
    pub owned_ptrs: Vec<(String, OwnedKind)>,
    /// Current impl block's target type (set when compiling impl methods).
    /// Used by compile_field_access to resolve `self.field` without scanning all impl_methods.
    pub current_impl_type: Option<String>,
    /// Functions that return fixed-size arrays: fn_name → (array_len, elem_type).
    pub fn_array_returns: &'a HashMap<String, (usize, cranelift_codegen::ir::Type)>,
    /// True when the last expression produced a heap array (e.g., str.chars()).
    pub last_heap_array: bool,
    /// When the last expression was a string split(), holds the opaque result pointer.
    pub last_split_result: Option<ClifValue>,
    /// Names of variables that hold split() results (opaque string arrays).
    pub split_vars: HashSet<String>,
    /// Functions that return strings (two return values: ptr, len).
    pub fn_returns_string: &'a HashSet<String>,
    /// Functions that return a heap-allocated dynamic array (Slice type).
    pub fn_returns_heap_array: &'a HashSet<String>,
    /// Functions that return a closure handle (closure with captures).
    pub fn_returns_closure_handle: &'a HashSet<String>,
    /// Functions that return a struct type: fn_name → struct_name.
    pub fn_returns_struct: &'a HashMap<String, String>,
    /// Maps closure variable names to their generated function names.
    pub closure_fn_map: &'a HashMap<String, String>,
    /// Maps closure function names to their list of captured variable names.
    pub closure_captures: &'a HashMap<String, Vec<String>>,
    /// Function pointer variables: var name → (param_types, return_type).
    pub fn_ptr_sigs: HashMap<
        String,
        (
            Vec<cranelift_codegen::ir::Type>,
            Option<cranelift_codegen::ir::Type>,
        ),
    >,
    /// Maps closure span (start, end) to generated function name for inline closures.
    pub closure_span_to_fn: HashMap<(usize, usize), String>,
    /// Variables that hold closure handles (returned closures with captures).
    pub closure_handle_vars: HashSet<String>,
    /// True when the last expression was a closure handle (returned from function).
    pub last_closure_handle: bool,
    /// Current module prefix for intra-module call resolution (e.g., "math").
    pub current_module: Option<String>,
    /// Names of variables that hold thread handles (from thread::spawn).
    pub thread_handles: HashSet<String>,
    /// True when the last expression was a thread::spawn call.
    pub last_thread_spawn: bool,
    /// Names of variables that hold Mutex handles.
    pub mutex_handles: HashSet<String>,
    /// True when the last expression was a Mutex::new() call.
    pub last_mutex_new: bool,
    /// Names of variables that hold channel handles.
    pub channel_handles: HashSet<String>,
    /// True when the last expression was a channel::new() call.
    pub last_channel_new: bool,
    /// Names of variables that hold atomic handles.
    pub atomic_handles: HashSet<String>,
    /// True when the last expression was an Atomic::new() call.
    pub last_atomic_new: bool,
    /// Subtype of the last atomic created: "i32", "bool", or "i64" (default).
    pub last_atomic_subtype: String,
    /// Maps variable name → atomic subtype ("i32", "bool", or "i64").
    pub atomic_subtypes: std::collections::HashMap<String, String>,
    /// Names of variables that hold RwLock handles.
    pub rwlock_handles: HashSet<String>,
    /// True when the last expression was a RwLock::new() call.
    pub last_rwlock_new: bool,
    /// Names of variables that hold Barrier handles.
    pub barrier_handles: HashSet<String>,
    /// True when the last expression was a Barrier::new() call.
    pub last_barrier_new: bool,
    /// Names of variables that hold Condvar handles.
    pub condvar_handles: HashSet<String>,
    /// True when the last expression was a Condvar::new() call.
    pub last_condvar_new: bool,
    /// Names of variables that hold bounded channel handles.
    pub bounded_channel_handles: HashSet<String>,
    /// True when the last expression was a channel::bounded() call.
    pub last_bounded_channel: bool,
    /// Names of variables that hold Arc handles.
    pub arc_handles: HashSet<String>,
    /// True when the last expression was an Arc::new() call.
    pub last_arc_new: bool,
    /// Generic function param mapping: fn_name → Vec of (param_index, generic_param_name).
    /// Used during call compilation to infer per-param type suffixes for multi-param generics.
    pub generic_fn_params: HashMap<String, Vec<(usize, String)>>,
    /// Set of async function names (their return is wrapped in a future handle).
    #[allow(dead_code)]
    pub async_fns: &'a HashSet<String>,
    /// Names of variables that hold future handles from async function calls.
    #[allow(dead_code)]
    pub future_handles: HashSet<String>,
    /// True when the last expression was an async function call.
    #[allow(dead_code)]
    pub last_future_new: bool,
    /// When true, disables IO/heap operations (bare metal mode).
    pub no_std: bool,
    /// User-defined panic handler function name (set by @panic_handler annotation).
    pub panic_handler_fn: Option<String>,
    /// Names of variables that hold VolatilePtr handles (just raw addresses).
    pub volatile_ptr_handles: HashSet<String>,
    /// True when the last expression was a VolatilePtr::new() call.
    pub last_volatile_ptr_new: bool,
    /// MMIO regions: var name → (base_var, size_var).
    pub mmio_regions: HashMap<String, (Variable, Variable)>,
    /// True when the last expression was an MmioRegion::new() call.
    pub last_mmio_new: bool,
    /// Temporarily holds (base_val, size_val) from the last MmioRegion::new().
    pub last_mmio_vals: Option<(ClifValue, ClifValue)>,
    /// Names of variables that hold BumpAllocator handles.
    pub bump_alloc_handles: HashSet<String>,
    /// True when the last expression was a BumpAllocator::new() call.
    pub last_bump_alloc_new: bool,
    /// Names of variables that hold FreeListAllocator handles.
    pub freelist_alloc_handles: HashSet<String>,
    /// True when the last expression was a FreeListAllocator::new() call.
    pub last_freelist_alloc_new: bool,
    /// Names of variables that hold PoolAllocator handles.
    pub pool_alloc_handles: HashSet<String>,
    /// True when the last expression was a PoolAllocator::new() call.
    pub last_pool_alloc_new: bool,
    /// Names of variables that hold Executor handles.
    pub executor_handles: HashSet<String>,
    /// True when the last expression was an Executor::new() call.
    pub last_executor_new: bool,
    /// Names of variables that hold Waker handles.
    pub waker_handles: HashSet<String>,
    /// True when the last expression was a Waker::new() call.
    pub last_waker_new: bool,
    /// Names of variables that hold TimerWheel handles.
    pub timer_handles: HashSet<String>,
    /// True when the last expression was a Timer::new() call.
    pub last_timer_new: bool,
    /// Names of variables that hold ThreadPool handles.
    pub threadpool_handles: HashSet<String>,
    /// True when the last expression was a ThreadPool::new() call.
    pub last_threadpool_new: bool,
    /// Names of variables that hold JoinHandle handles.
    pub joinhandle_handles: HashSet<String>,
    /// True when the last expression was a spawn_join() call.
    pub last_joinhandle_new: bool,
    /// Names of variables that hold async channel handles.
    pub async_channel_handles: HashSet<String>,
    /// True when the last expression was an AsyncChannel::new() call.
    pub last_async_channel_new: bool,
    /// Names of variables that hold async bounded channel handles.
    pub async_bchannel_handles: HashSet<String>,
    /// True when the last expression was an AsyncChannel::bounded() call.
    pub last_async_bchannel_new: bool,
    /// Names of variables that hold Stream handles.
    pub stream_handles: HashSet<String>,
    /// True when the last expression was a Stream::new/from_range/map/filter/take call.
    pub last_stream_new: bool,
    /// Names of variables that hold SIMD f32x4 handles.
    pub simd_f32x4_handles: HashSet<String>,
    /// True when the last expression was a f32x4 constructor.
    pub last_simd_f32x4_new: bool,
    /// Names of variables that hold SIMD i32x4 handles.
    pub simd_i32x4_handles: HashSet<String>,
    /// True when the last expression was a i32x4 constructor.
    pub last_simd_i32x4_new: bool,
    /// Names of variables that hold SIMD f32x8 handles.
    pub simd_f32x8_handles: HashSet<String>,
    /// True when the last expression was a f32x8 constructor.
    pub last_simd_f32x8_new: bool,
    /// Names of variables that hold SIMD i32x8 handles.
    pub simd_i32x8_handles: HashSet<String>,
    /// True when the last expression was a i32x8 constructor.
    pub last_simd_i32x8_new: bool,
    /// Names of variables that hold ONNX model builder handles.
    pub onnx_handles: HashSet<String>,
    /// True when the last expression was an OnnxModel::new() call.
    pub last_onnx_new: bool,
    /// Names of variables that hold async I/O handles (from async_read_file/async_write_file).
    pub async_io_handles: HashSet<String>,
    /// True when the last expression was an async_read_file/async_write_file call.
    pub last_async_io_new: bool,
    /// True when the last expression was a call to a function that returns a heap array.
    pub last_heap_array_return: bool,
    /// Declared return type of the current function (for return value coercion).
    pub fn_ret_type: Option<cranelift_codegen::ir::Type>,
}

/// Emits cleanup code (free calls) for all owned heap resources.
///
/// If `returned_val` is `Some(v)`, skips freeing any resource whose current
/// pointer value equals `v` (ownership transferred to caller).
pub(crate) fn emit_owned_cleanup<M: Module>(
    builder: &mut FunctionBuilder,
    cx: &mut CodegenCtx<'_, M>,
    returned_val: Option<ClifValue>,
) -> Result<(), CodegenError> {
    // Take ownership of the list to avoid borrow issues
    let owned = std::mem::take(&mut cx.owned_ptrs);
    // Track freed SSA values to avoid double-free when multiple variables alias
    // the same heap pointer (e.g., `a = b` makes both point to b's allocation).
    let mut freed_vals = Vec::new();
    for (name, kind) in &owned {
        let var = match cx.var_map.get(name) {
            Some(v) => *v,
            None => continue,
        };
        let ptr = builder.use_var(var);

        // If this pointer is the returned value, skip freeing it (ownership transfer)
        if let Some(ret_val) = returned_val {
            if ptr == ret_val {
                continue;
            }
        }

        // Skip if this SSA value was already freed by a previous iteration
        if freed_vals.contains(&ptr) {
            continue;
        }
        freed_vals.push(ptr);

        match kind {
            OwnedKind::String => {
                // Need the length to call fj_rt_free(ptr, len)
                if let Some(len_var) = cx.string_lens.get(name) {
                    let len = builder.use_var(*len_var);
                    let free_id = cx
                        .functions
                        .get("__free")
                        .ok_or_else(|| CodegenError::Internal("__free not declared".into()))?;
                    let local_callee = cx.module.declare_func_in_func(*free_id, builder.func);
                    builder.ins().call(local_callee, &[ptr, len]);
                }
            }
            OwnedKind::Array => {
                let free_id = cx
                    .functions
                    .get("__array_free")
                    .ok_or_else(|| CodegenError::Internal("__array_free not declared".into()))?;
                let local_callee = cx.module.declare_func_in_func(*free_id, builder.func);
                builder.ins().call(local_callee, &[ptr]);
            }
            OwnedKind::Map => {
                let free_id = cx
                    .functions
                    .get("__map_free")
                    .ok_or_else(|| CodegenError::Internal("__map_free not declared".into()))?;
                let local_callee = cx.module.declare_func_in_func(*free_id, builder.func);
                builder.ins().call(local_callee, &[ptr]);
            }
            OwnedKind::BumpAllocator => {
                if let Some(&free_id) = cx.functions.get("__bump_destroy") {
                    let local_callee = cx.module.declare_func_in_func(free_id, builder.func);
                    builder.ins().call(local_callee, &[ptr]);
                }
            }
            OwnedKind::FreeListAllocator => {
                if let Some(&free_id) = cx.functions.get("__freelist_destroy") {
                    let local_callee = cx.module.declare_func_in_func(free_id, builder.func);
                    builder.ins().call(local_callee, &[ptr]);
                }
            }
            OwnedKind::PoolAllocator => {
                if let Some(&free_id) = cx.functions.get("__pool_destroy") {
                    let local_callee = cx.module.declare_func_in_func(free_id, builder.func);
                    builder.ins().call(local_callee, &[ptr]);
                }
            }
        }
    }
    cx.owned_ptrs = owned;
    Ok(())
}
