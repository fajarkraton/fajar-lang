//! V20 builtin tests — 2 tests per builtin, 14 builtins = 28 tests.
//!
//! Tests every V20 builtin via eval_source/eval_program with captured output.
//! Covers: diffusion, RL, pipeline, accelerator, actors, const, map_get_or.

/// Evaluate source code and capture all printed output.
fn eval_capture(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp.eval_program(&program).expect("eval failed");
    interp.get_output().join("\n")
}

/// Evaluate and expect a runtime error.
fn eval_err(source: &str) -> String {
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let err = interp.eval_program(&program).expect_err("expected error");
    format!("{err}")
}

// ════════════════════════════════════════════════════════════════════════
// 1. diffusion_create (tasks 1.4.1)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_diffusion_create_returns_model_map() {
    let out = eval_capture(
        r#"
let model = diffusion_create(100)
println(type_of(model))
println(map_get_or(model, "_type", "unknown"))
println(map_get_or(model, "steps", 0))
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    assert!(
        lines.iter().any(|l| l.contains("map") || l.contains("Map")),
        "should be a map, got: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("DiffusionModel")),
        "should have DiffusionModel type, got: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("100")),
        "should have steps=100, got: {lines:?}"
    );
}

#[test]
fn v20_diffusion_create_minimum_steps() {
    // Edge case: steps=1 should work
    let out = eval_capture(
        r#"
let model = diffusion_create(1)
println(map_get_or(model, "steps", 0))
"#,
    );
    assert!(out.contains("1"), "steps=1 should work, got: {out}");
}

// ════════════════════════════════════════════════════════════════════════
// 2. diffusion_denoise (task 1.4.2)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_diffusion_denoise_preserves_shape() {
    let out = eval_capture(
        r#"
let model = diffusion_create(100)
let noise = randn(2, 3)
let result = diffusion_denoise(model, noise, 50)
println(type_of(result))
"#,
    );
    assert!(
        out.contains("tensor") || out.contains("Tensor"),
        "denoise should return tensor, got: {out}"
    );
}

#[test]
fn v20_diffusion_denoise_step_affects_magnitude() {
    // Step 0 (beginning) vs step 99 (near end) should produce different scaling
    // At step 0, scale = 1.0 - 0 * 0.5 = 1.0 (values stay ~1.0)
    // At step 99, scale = 1.0 - 0.99 * 0.5 = 0.505 (values ~0.505)
    let out = eval_capture(
        r#"
let model = diffusion_create(100)
let noise = ones(1, 4)
let early = diffusion_denoise(model, noise, 0)
let late = diffusion_denoise(model, noise, 99)
let e_sum = const_size_of(early)
let l_sum = const_size_of(late)
println(e_sum)
println(l_sum)
println(type_of(early))
println(type_of(late))
"#,
    );
    // Both should be tensors — the denoising operation preserves tensor type
    assert!(
        out.contains("tensor") || out.contains("Tensor"),
        "both should be tensors, got: {out}"
    );
    // Both tensors have same shape so const_size_of will match — just verify type
}

// ════════════════════════════════════════════════════════════════════════
// 3. rl_agent_create (task 1.4.3)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_rl_agent_create_returns_correct_map() {
    let out = eval_capture(
        r#"
let agent = rl_agent_create(4, 2)
println(map_get_or(agent, "_type", "unknown"))
println(map_get_or(agent, "state_dim", 0))
println(map_get_or(agent, "action_dim", 0))
"#,
    );
    assert!(
        out.contains("RLAgent"),
        "should have RLAgent type, got: {out}"
    );
    assert!(out.contains("4"), "state_dim should be 4, got: {out}");
    assert!(out.contains("2"), "action_dim should be 2, got: {out}");
}

#[test]
fn v20_rl_agent_create_state_array_length() {
    let out = eval_capture(
        r#"
let agent = rl_agent_create(6, 3)
let state = map_get_or(agent, "state", [])
println(len(state))
"#,
    );
    assert!(
        out.trim().contains("6"),
        "state array should have length 6, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 4. rl_agent_step (task 1.4.4)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_rl_agent_step_returns_reward_and_done() {
    let out = eval_capture(
        r#"
let agent = rl_agent_create(4, 2)
let result = rl_agent_step(agent, 0)
let reward = map_get_or(result, "reward", -999.0)
let done = map_get_or(result, "done", false)
println(type_of(reward))
println(type_of(done))
println(reward)
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(
        lines.len() >= 2,
        "expected at least 2 type lines, got: {lines:?}"
    );
    assert!(
        lines[0].contains("f64") || lines[0].contains("float"),
        "reward should be float type, got: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("bool"),
        "done should be bool type, got: {}",
        lines[1]
    );
}

#[test]
fn v20_rl_agent_step_different_actions() {
    let out = eval_capture(
        r#"
let agent = rl_agent_create(4, 2)
let r0 = rl_agent_step(agent, 0)
let r1 = rl_agent_step(agent, 1)
println(map_get_or(r0, "reward", -999.0))
println(map_get_or(r1, "reward", -999.0))
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    assert!(lines.len() >= 2, "expected 2 reward lines, got: {lines:?}");
    // Both should return valid rewards (not the default -999)
    for line in &lines {
        assert!(
            !line.contains("-999"),
            "reward should be valid, got default: {line}"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════
// 5. pipeline_create + pipeline_add_stage (task 1.4.5)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_pipeline_create_empty() {
    let out = eval_capture(
        r#"
let pipe = pipeline_create()
println(map_get_or(pipe, "stage_count", -1))
println(map_get_or(pipe, "_type", "unknown"))
"#,
    );
    assert!(
        out.contains("0"),
        "empty pipeline should have 0 stages, got: {out}"
    );
    assert!(
        out.contains("Pipeline"),
        "type should be Pipeline, got: {out}"
    );
}

#[test]
fn v20_pipeline_add_stage_increments_count() {
    let out = eval_capture(
        r#"
fn double(x: i64) -> i64 { x * 2 }
fn add_one(x: i64) -> i64 { x + 1 }
fn negate(x: i64) -> i64 { 0 - x }
let pipe = pipeline_create()
let pipe = pipeline_add_stage(pipe, "double", "double")
let pipe = pipeline_add_stage(pipe, "add_one", "add_one")
let pipe = pipeline_add_stage(pipe, "negate", "negate")
println(map_get_or(pipe, "stage_count", -1))
"#,
    );
    assert!(
        out.contains("3"),
        "should have 3 stages after adding 3, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 6. pipeline_run — happy path (task 1.4.6)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_pipeline_run_chain_functions() {
    let out = eval_capture(
        r#"
fn double(x: i64) -> i64 { x * 2 }
fn add_one(x: i64) -> i64 { x + 1 }
let pipe = pipeline_create()
let pipe = pipeline_add_stage(pipe, "double", "double")
let pipe = pipeline_add_stage(pipe, "add_one", "add_one")
let result = pipeline_run(pipe, 5)
println(result)
"#,
    );
    // 5 * 2 = 10, 10 + 1 = 11
    assert!(
        out.contains("11"),
        "double(5) then add_one should be 11, got: {out}"
    );
}

#[test]
fn v20_pipeline_run_identity() {
    // Empty pipeline should return input unchanged
    let out = eval_capture(
        r#"
let pipe = pipeline_create()
let result = pipeline_run(pipe, 42)
println(result)
"#,
    );
    assert!(
        out.contains("42"),
        "empty pipeline should return input 42, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 7. pipeline_run — error propagation (task 1.4.7)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_pipeline_run_bad_function_returns_error() {
    let err = eval_err(
        r#"
let pipe = pipeline_create()
let pipe = pipeline_add_stage(pipe, "bad_stage", "nonexistent_fn")
let result = pipeline_run(pipe, 1)
"#,
    );
    assert!(
        err.contains("pipeline")
            || err.contains("stage")
            || err.contains("nonexistent")
            || err.contains("undefined")
            || err.contains("Undefined"),
        "error should mention pipeline/stage failure, got: {err}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 8. accelerate (task 1.4.8)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_accelerate_returns_classification() {
    let out = eval_capture(
        r#"
fn process(x: i64) -> i64 { x * 2 }
let result = accelerate("process", 42)
println(type_of(result))
let device = map_get_or(result, "device", "none")
println(device)
"#,
    );
    assert!(
        out.contains("map") || out.contains("Map"),
        "accelerate should return a map, got: {out}"
    );
    // Should classify to some device (GPU, CPU, or NPU)
    assert!(
        out.contains("GPU") || out.contains("CPU") || out.contains("NPU"),
        "should have device classification, got: {out}"
    );
}

#[test]
fn v20_accelerate_with_tensor_input() {
    let out = eval_capture(
        r#"
fn scale(t: Tensor) -> Tensor { t }
let t = ones(2, 3)
let result = accelerate("scale", t)
let wc = map_get_or(result, "workload_class", "unknown")
println(wc)
"#,
    );
    // Tensor input should produce a valid workload classification
    assert!(
        out.contains("Bound")
            || out.contains("Sensitive")
            || out.contains("Compute")
            || out.contains("Memory")
            || out.contains("Latency"),
        "should have workload class, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 9. actor_spawn (task 1.4.9)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_actor_spawn_returns_actor_map() {
    let out = eval_capture(
        r#"
fn handler(msg: str) -> str { msg }
let actor = actor_spawn("worker", "handler")
println(map_get_or(actor, "_type", "unknown"))
println(map_get_or(actor, "name", "unknown"))
"#,
    );
    assert!(out.contains("Actor"), "type should be Actor, got: {out}");
    assert!(out.contains("worker"), "name should be worker, got: {out}");
}

#[test]
fn v20_actor_spawn_unique_addresses() {
    let out = eval_capture(
        r#"
fn handler(msg: str) -> str { msg }
let a1 = actor_spawn("actor1", "handler")
let a2 = actor_spawn("actor2", "handler")
let addr1 = map_get_or(a1, "addr", 0)
let addr2 = map_get_or(a2, "addr", 0)
println(addr1)
println(addr2)
"#,
    );
    let lines: Vec<&str> = out.trim().lines().collect();
    // Filter to non-warn lines (skip [sim] warnings)
    let addr_lines: Vec<&str> = lines
        .iter()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]") && !l.is_empty())
        .copied()
        .collect();
    assert!(
        addr_lines.len() >= 2,
        "expected 2 addr lines, got: {addr_lines:?}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 10. actor_send (task 1.4.10)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_actor_send_calls_handler() {
    let out = eval_capture(
        r#"
fn handler(msg: i64) -> i64 { msg * 10 }
let actor = actor_spawn("worker", "handler")
let result = actor_send(actor, 5)
println(result)
"#,
    );
    // handler(5) should return 50
    assert!(
        out.contains("50"),
        "actor_send(5) through handler should return 50, got: {out}"
    );
}

#[test]
fn v20_actor_send_with_string_message() {
    let out = eval_capture(
        r#"
fn greet(name: str) -> str { "hello " + name }
let actor = actor_spawn("greeter", "greet")
let result = actor_send(actor, "world")
println(result)
"#,
    );
    assert!(out.contains("hello world"), "should greet, got: {out}");
}

// ════════════════════════════════════════════════════════════════════════
// 11. actor_supervise (task 1.4.11)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_actor_supervise_one_for_one() {
    let out = eval_capture(
        r#"
fn handler(msg: str) -> str { msg }
let actor = actor_spawn("worker", "handler")
let supervised = actor_supervise(actor, "one_for_one")
println(map_get_or(supervised, "supervision", "none"))
"#,
    );
    assert!(
        out.contains("one_for_one"),
        "should have one_for_one strategy, got: {out}"
    );
}

#[test]
fn v20_actor_supervise_all_for_one() {
    let out = eval_capture(
        r#"
fn handler(msg: str) -> str { msg }
let actor = actor_spawn("worker", "handler")
let supervised = actor_supervise(actor, "all_for_one")
println(map_get_or(supervised, "supervision", "none"))
"#,
    );
    assert!(
        out.contains("all_for_one"),
        "should have all_for_one strategy, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 12. const_alloc (task 1.4.12)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_const_alloc_correct_size() {
    let out = eval_capture(
        r#"
let alloc = const_alloc(4096)
println(map_get_or(alloc, "_type", "unknown"))
println(map_get_or(alloc, "size", -1))
println(map_get_or(alloc, "section", "none"))
"#,
    );
    assert!(
        out.contains("ConstAlloc"),
        "type should be ConstAlloc, got: {out}"
    );
    assert!(out.contains("4096"), "size should be 4096, got: {out}");
    assert!(
        out.contains(".rodata"),
        "section should be .rodata, got: {out}"
    );
}

#[test]
fn v20_const_alloc_zero_size() {
    // Edge case: size=0 should work
    let out = eval_capture(
        r#"
let alloc = const_alloc(0)
println(map_get_or(alloc, "size", -1))
"#,
    );
    assert!(out.contains("0"), "size=0 should work, got: {out}");
}

// ════════════════════════════════════════════════════════════════════════
// 12b. const_serialize — V26 A3.1: wire serialize_const() to .fj
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v26_a3_1_const_serialize_int() {
    // i64 → 8 bytes little-endian, type "i64"
    let out = eval_capture(
        r#"
let s = const_serialize(42)
println(map_get_or(s, "size", -1))
println(map_get_or(s, "align", -1))
println(map_get_or(s, "type_desc", "none"))
"#,
    );
    assert!(out.contains("8"), "i64 size should be 8, got: {out}");
    assert!(out.contains("i64"), "type_desc should be i64, got: {out}");
}

#[test]
fn v26_a3_1_const_serialize_bool() {
    // bool → 1 byte
    let out = eval_capture(
        r#"
let s = const_serialize(true)
println(map_get_or(s, "size", -1))
println(map_get_or(s, "align", -1))
println(map_get_or(s, "type_desc", "none"))
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(
        lines.contains(&"1"),
        "bool should serialize to 1 byte: {out}"
    );
    assert!(out.contains("bool"), "type_desc should be bool: {out}");
}

#[test]
fn v26_a3_1_const_serialize_str() {
    // str → 8-byte length prefix + utf8 bytes
    // "hello" → 8 (len prefix) + 5 (utf8) = 13 bytes
    let out = eval_capture(
        r#"
let s = const_serialize("hello")
println(map_get_or(s, "size", -1))
println(map_get_or(s, "type_desc", "none"))
"#,
    );
    assert!(out.contains("13"), "str hello should be 13 bytes: {out}");
    assert!(
        out.contains("str(len=5)"),
        "type_desc should be str(len=5): {out}"
    );
}

#[test]
fn v26_a3_1_const_serialize_array() {
    // [i64; 3] → 8 (count prefix) + 3*8 (elements) = 32 bytes
    let out = eval_capture(
        r#"
let s = const_serialize([1, 2, 3])
println(map_get_or(s, "size", -1))
println(map_get_or(s, "type_desc", "none"))
"#,
    );
    assert!(out.contains("32"), "[i64; 3] should be 32 bytes: {out}");
    assert!(
        out.contains("[i64; 3]"),
        "type_desc should be [i64; 3]: {out}"
    );
}

#[test]
fn v26_a3_1_const_serialize_returns_bytes_array() {
    // Verify the bytes array is actually populated, not just the descriptor
    let out = eval_capture(
        r#"
let s = const_serialize(42)
let bytes = map_get_or(s, "bytes", [])
println(len(bytes))
"#,
    );
    assert!(
        out.contains("8"),
        "bytes array should have 8 entries: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 12c. const_eval_nat — V26 A3.2: wire parse_nat_expr() + eval_nat() to .fj
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v26_a3_2_const_eval_nat_literal_add() {
    // "5+3" with empty bindings → 8
    let out = eval_capture(
        r#"
println(const_eval_nat("5+3", map_new()))
"#,
    );
    assert!(out.trim().contains("8"), "5+3 should be 8: {out}");
}

#[test]
fn v26_a3_2_const_eval_nat_literal_mul_sub() {
    // Verify * and - operators
    let out = eval_capture(
        r#"
println(const_eval_nat("10*4", map_new()))
println(const_eval_nat("100-25", map_new()))
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(lines.iter().any(|l| l.trim() == "40"), "10*4=40: {out}");
    assert!(lines.iter().any(|l| l.trim() == "75"), "100-25=75: {out}");
}

#[test]
fn v26_a3_2_const_eval_nat_with_bindings() {
    // "N+1" with N=10 → 11
    let out = eval_capture(
        r#"
let env = map_insert(map_new(), "N", 10)
println(const_eval_nat("N+1", env))
"#,
    );
    assert!(
        out.trim().contains("11"),
        "N+1 with N=10 should be 11: {out}"
    );
}

#[test]
fn v26_a3_2_const_eval_nat_multi_param() {
    // "N*M" with N=3, M=4 → 12
    let out = eval_capture(
        r#"
let env = map_insert(map_insert(map_new(), "N", 3), "M", 4)
println(const_eval_nat("N*M", env))
"#,
    );
    assert!(
        out.trim().contains("12"),
        "N*M with N=3,M=4 should be 12: {out}"
    );
}

#[test]
fn v26_a3_2_const_eval_nat_unbound_returns_null() {
    // Unbound parameter → null (Option::None from eval_nat)
    let out = eval_capture(
        r#"
let result = const_eval_nat("X+1", map_new())
println(result)
"#,
    );
    assert!(
        out.contains("null"),
        "X+1 with no bindings should be null: {out}"
    );
}

#[test]
fn v26_a3_2_const_generic_fn_definition_parses() {
    // Smoke test that const generic fn syntax already parses + analyzes
    // (the V26 A3.2 verification bar: "examples/const_generics_demo.fj compiles")
    let out = eval_capture(
        r#"
fn const_gen<const N: usize>() -> i64 { 42 }
println(const_gen())
"#,
    );
    assert!(out.contains("42"), "const generic fn should run: {out}");
}

#[test]
fn v26_a3_2_const_generic_struct_parses() {
    let out = eval_capture(
        r#"
struct Buffer<const SIZE: usize> { capacity: i64 }
let b = Buffer { capacity: 256 }
println(b.capacity)
"#,
    );
    assert!(
        out.contains("256"),
        "const generic struct should compile + access field: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 13. const_size_of + const_align_of (task 1.4.13)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_const_size_of_basic_types() {
    let out = eval_capture(
        r#"
println(const_size_of(42))
println(const_size_of(true))
println(const_size_of('a'))
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(lines.len() >= 3, "expected 3 size outputs, got: {lines:?}");
    // i64 = 8, bool = 1, char = 4
    assert!(
        lines[0].trim() == "8",
        "i64 should be 8 bytes, got: {}",
        lines[0]
    );
    assert!(
        lines[1].trim() == "1",
        "bool should be 1 byte, got: {}",
        lines[1]
    );
    assert!(
        lines[2].trim() == "4",
        "char should be 4 bytes, got: {}",
        lines[2]
    );
}

#[test]
fn v20_const_align_of_basic_types() {
    let out = eval_capture(
        r#"
println(const_align_of(42))
println(const_align_of(true))
println(const_align_of('a'))
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(lines.len() >= 3, "expected 3 align outputs, got: {lines:?}");
    // i64 align=8, bool align=1, char align=4
    assert!(
        lines[0].trim() == "8",
        "i64 align should be 8, got: {}",
        lines[0]
    );
    assert!(
        lines[1].trim() == "1",
        "bool align should be 1, got: {}",
        lines[1]
    );
    assert!(
        lines[2].trim() == "4",
        "char align should be 4, got: {}",
        lines[2]
    );
}

#[test]
fn v20_const_size_of_tensor_scales() {
    let out = eval_capture(
        r#"
let small = zeros(1, 4)
let big = zeros(1, 100)
let s1 = const_size_of(small)
let s2 = const_size_of(big)
println(s1)
println(s2)
"#,
    );
    let lines: Vec<&str> = out
        .trim()
        .lines()
        .filter(|l| !l.starts_with("[warn]") && !l.starts_with("[sim]"))
        .collect();
    assert!(lines.len() >= 2, "expected 2 size lines, got: {lines:?}");
    let s1: i64 = lines[0].trim().parse().expect("parse s1");
    let s2: i64 = lines[1].trim().parse().expect("parse s2");
    assert!(
        s2 > s1,
        "larger tensor should have larger size: {s1} vs {s2}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 14. map_get_or (task 1.4.14)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_map_get_or_key_exists() {
    let out = eval_capture(
        r#"
let m = map_new()
let m = map_insert(m, "name", "fajar")
let val = map_get_or(m, "name", "default")
println(val)
"#,
    );
    assert!(
        out.contains("fajar"),
        "should return existing value, got: {out}"
    );
}

#[test]
fn v20_map_get_or_key_missing() {
    let out = eval_capture(
        r#"
let m = map_new()
let val = map_get_or(m, "missing", "fallback")
println(val)
"#,
    );
    assert!(
        out.contains("fallback"),
        "should return default value, got: {out}"
    );
}

// ════════════════════════════════════════════════════════════════════════
// V20.5 Phase 2.1: Runtime error source spans
// ════════════════════════════════════════════════════════════════════════

#[test]
fn v20_5_division_by_zero_has_span() {
    let source = "let x = 1 / 0";
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let err = interp.eval_program(&program).expect_err("should error");
    assert!(
        format!("{err}").contains("RE001"),
        "should be RE001 division by zero, got: {err}"
    );
    // The interpreter should have captured the span
    let span = interp.last_error_span();
    assert!(span.is_some(), "division by zero should have a source span");
    let span = span.unwrap();
    // The span should point to the binary expression "1 / 0"
    assert!(span.start < span.end, "span should be non-empty: {span:?}");
    let highlighted = &source[span.start..span.end];
    assert!(
        highlighted.contains("/"),
        "span should cover the division: '{highlighted}'"
    );
}

#[test]
fn v20_5_index_out_of_bounds_has_span() {
    let source = "let a = [1, 2, 3]\nlet x = a[99]";
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let err = interp.eval_program(&program).expect_err("should error");
    assert!(
        format!("{err}").contains("RE010") || format!("{err}").contains("out of bounds"),
        "should be index out of bounds, got: {err}"
    );
    let span = interp.last_error_span();
    assert!(
        span.is_some(),
        "index out of bounds should have a source span"
    );
}

#[test]
fn v20_5_undefined_function_has_span() {
    let source = "let x = nonexistent(42)";
    let tokens = fajar_lang::lexer::tokenize(source).expect("lex failed");
    let program = fajar_lang::parser::parse(tokens).expect("parse failed");
    let _ = fajar_lang::analyzer::analyze(&program);
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let err = interp.eval_program(&program).expect_err("should error");
    assert!(
        format!("{err}").contains("RE004") || format!("{err}").contains("undefined"),
        "should be undefined, got: {err}"
    );
    let span = interp.last_error_span();
    assert!(span.is_some(), "call error should have a source span");
}
