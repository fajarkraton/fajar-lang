//! Integration tests for B5.L7 — QuantizedKVCache.

use fajar_lang::interpreter::Interpreter;

fn run_ok(src: &str) -> String {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => format!("{val}"),
        Err(e) => panic!("expected OK, got error: {e}"),
    }
}

fn run_err(src: &str, substr: &str) {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => panic!("expected error containing '{substr}', got OK: {val}"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(msg.contains(substr), "expected '{substr}' in: {msg}");
        }
    }
}

#[test]
fn create_empty_cache() {
    let result = run_ok(
        r#"
        let cache = kv_cache_create(128, 4, 2)
        kv_cache_len(cache)
    "#,
    );
    assert_eq!(result, "0");
}

#[test]
fn create_cache_zero_size_bytes() {
    let result = run_ok(
        r#"
        let cache = kv_cache_create(128, 4, 2)
        kv_cache_size_bytes(cache)
    "#,
    );
    assert_eq!(result, "0");
}

#[test]
fn update_and_query() {
    let result = run_ok(
        r#"
        let mut cache = kv_cache_create(64, 2, 4)
        let k = quantize(from_data([1.0, 2.0, 3.0, 4.0], [1, 4]), 4)
        let v = quantize(from_data([5.0, 6.0, 7.0, 8.0], [1, 4]), 4)
        cache = kv_cache_update(cache, 0, k, v)
        let keys = kv_cache_get_keys(cache, 0)
        len(keys)
    "#,
    );
    assert_eq!(result, "1");
}

#[test]
fn update_multiple_tokens() {
    let result = run_ok(
        r#"
        let mut cache = kv_cache_create(64, 1, 4)
        let mut i = 0
        while i < 5 {
            let k = quantize(from_data([1.0, 2.0, 3.0, 4.0], [1, 4]), 4)
            let v = quantize(from_data([5.0, 6.0, 7.0, 8.0], [1, 4]), 4)
            cache = kv_cache_update(cache, 0, k, v)
            i = i + 1
        }
        let keys = kv_cache_get_keys(cache, 0)
        len(keys)
    "#,
    );
    assert_eq!(result, "5");
}

#[test]
fn size_bytes_grows() {
    let result = run_ok(
        r#"
        let mut cache = kv_cache_create(64, 1, 4)
        let k = quantize(from_data([1.0, 2.0, 3.0, 4.0], [1, 4]), 4)
        let v = quantize(from_data([5.0, 6.0, 7.0, 8.0], [1, 4]), 4)
        cache = kv_cache_update(cache, 0, k, v)
        kv_cache_size_bytes(cache)
    "#,
    );
    let bytes: i64 = result.parse().unwrap();
    assert!(bytes > 0, "size should be > 0 after update, got {bytes}");
}

#[test]
fn overflow_detection() {
    run_err(
        r#"
        let mut cache = kv_cache_create(2, 1, 4)
        let k = quantize(from_data([1.0, 2.0], [1, 2]), 4)
        let v = quantize(from_data([3.0, 4.0], [1, 2]), 4)
        cache = kv_cache_update(cache, 0, k, v)
        cache = kv_cache_update(cache, 0, k, v)
        cache = kv_cache_update(cache, 0, k, v)
    "#,
        "overflow",
    );
}

#[test]
fn get_values_layer() {
    let result = run_ok(
        r#"
        let mut cache = kv_cache_create(64, 2, 4)
        let k = quantize(from_data([1.0, 2.0], [1, 2]), 4)
        let v = quantize(from_data([3.0, 4.0], [1, 2]), 4)
        cache = kv_cache_update(cache, 1, k, v)
        let vals = kv_cache_get_values(cache, 1)
        len(vals)
    "#,
    );
    assert_eq!(result, "1");
}

#[test]
fn layer_out_of_range() {
    run_err(
        r#"
        let cache = kv_cache_create(64, 2, 4)
        kv_cache_get_keys(cache, 5)
    "#,
        "out of range",
    );
}
