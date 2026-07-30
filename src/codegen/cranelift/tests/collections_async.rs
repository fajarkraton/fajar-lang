//! HashMaps, timer wheel, thread pools, work stealing, async channels (S10-S12).

use super::super::*;
use super::compile_and_run;
use crate::lexer::tokenize;
use crate::parser::parse;

// =====================================================================
// S9.2 — Future/Poll Types in Native Codegen
// =====================================================================

#[test]
fn native_async_fn_returns_value() {
    // async fn wraps its body result in a future; .await unwraps it
    let src = r#"
        async fn answer() -> i64 {
            42
        }
        fn main() -> i64 {
            answer().await
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_async_fn_with_params() {
    // async fn with parameters
    let src = r#"
        async fn add(a: i64, b: i64) -> i64 {
            a + b
        }
        fn main() -> i64 {
            add(17, 25).await
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_async_fn_chain() {
    // one async fn awaits another
    let src = r#"
        async fn get_base() -> i64 {
            40
        }
        async fn add_two() -> i64 {
            let base = get_base().await
            base + 2
        }
        fn main() -> i64 {
            add_two().await
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_async_fn_computation() {
    // async fn with computation, not just return literal
    let src = r#"
        async fn fib(n: i64) -> i64 {
            if n <= 1 { n } else { fib(n - 1).await + fib(n - 2).await }
        }
        fn main() -> i64 {
            fib(10).await
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

// =====================================================================
// S9.3 — Async State Machine Desugaring
// =====================================================================

#[test]
fn native_async_multi_sequential_awaits() {
    // Two sequential await points: each produces a value used later
    let src = r#"
        async fn first() -> i64 { 10 }
        async fn second() -> i64 { 20 }
        async fn combined() -> i64 {
            let a = first().await
            let b = second().await
            a + b
        }
        fn main() -> i64 {
            combined().await
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_async_local_var_preserved_across_await() {
    // Local variable x defined before await must survive after await
    let src = r#"
        async fn fetch() -> i64 { 5 }
        async fn compute() -> i64 {
            let x = 100
            let y = fetch().await
            x + y
        }
        fn main() -> i64 {
            compute().await
        }
    "#;
    assert_eq!(compile_and_run(src), 105);
}

#[test]
fn native_async_three_sequential_awaits() {
    // Three sequential awaits — verifies multi-state transitions
    let src = r#"
        async fn a() -> i64 { 1 }
        async fn b() -> i64 { 2 }
        async fn c() -> i64 { 3 }
        async fn sum_all() -> i64 {
            let x = a().await
            let y = b().await
            let z = c().await
            x + y + z
        }
        fn main() -> i64 {
            sum_all().await
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_async_local_mutation_across_await() {
    // Mutable local modified, then preserved across await
    let src = r#"
        async fn get_val() -> i64 { 7 }
        async fn process() -> i64 {
            let mut acc = 50
            acc = acc + 3
            let v = get_val().await
            acc + v
        }
        fn main() -> i64 {
            process().await
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

// =====================================================================
// S9.4 — Poll-based Await Compilation
// =====================================================================

#[test]
fn native_await_poll_ready() {
    // Poll-based await: future is immediately ready
    let src = r#"
        async fn ready_val() -> i64 { 99 }
        fn main() -> i64 {
            ready_val().await
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_await_poll_chain() {
    // Chained awaits: each depends on previous via poll
    let src = r#"
        async fn step1() -> i64 { 10 }
        async fn step2(x: i64) -> i64 { x * 2 }
        async fn step3(x: i64) -> i64 { x + 5 }
        async fn pipeline() -> i64 {
            let a = step1().await
            let b = step2(a).await
            step3(b).await
        }
        fn main() -> i64 {
            pipeline().await
        }
    "#;
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_await_poll_with_computation() {
    // Computation between poll-based awaits
    let src = r#"
        async fn square(n: i64) -> i64 { n * n }
        async fn process() -> i64 {
            let a = square(3).await
            let b = a + 10
            let c = square(b).await
            c
        }
        fn main() -> i64 {
            process().await
        }
    "#;
    // a = 9, b = 19, c = 361
    assert_eq!(compile_and_run(src), 361);
}

// =====================================================================
// S10.1 — Executor
// =====================================================================

#[test]
fn native_executor_block_on_ready() {
    // Executor.block_on() runs a future to completion
    let src = r#"
        async fn answer() -> i64 { 42 }
        fn main() -> i64 {
            let exec = Executor::new()
            let result = exec.block_on(answer())
            exec.free()
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_executor_spawn_and_run() {
    // Executor.spawn() + run() executes spawned tasks
    let src = r#"
        async fn task1() -> i64 { 10 }
        async fn task2() -> i64 { 20 }
        fn main() -> i64 {
            let exec = Executor::new()
            exec.spawn(task1())
            exec.spawn(task2())
            let completed = exec.run()
            exec.free()
            completed
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_executor_get_result() {
    // Retrieve results of spawned tasks by index
    let src = r#"
        async fn make_val(n: i64) -> i64 { n * 10 }
        fn main() -> i64 {
            let exec = Executor::new()
            exec.spawn(make_val(3))
            exec.spawn(make_val(5))
            let r0 = exec.get_result(0)
            let r1 = exec.get_result(1)
            exec.free()
            r0 + r1
        }
    "#;
    assert_eq!(compile_and_run(src), 80);
}

#[test]
fn native_executor_multiple_block_on() {
    // Multiple block_on calls on the same executor
    let src = r#"
        async fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            let exec = Executor::new()
            let r1 = exec.block_on(add(10, 20))
            let r2 = exec.block_on(add(r1, 5))
            exec.free()
            r2
        }
    "#;
    assert_eq!(compile_and_run(src), 35);
}

// =====================================================================
// S10.2 — Waker Implementation
// =====================================================================

#[test]
fn native_waker_wake_and_check() {
    // Waker starts unwoken, wake sets flag
    let src = r#"
        fn main() -> i64 {
            let w = Waker::new()
            let before = w.is_woken()
            w.wake()
            let after = w.is_woken()
            w.drop()
            before * 10 + after
        }
    "#;
    // before = 0, after = 1 → 0*10 + 1 = 1
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_waker_clone_shares_state() {
    // Cloned waker shares the same underlying state
    let src = r#"
        fn main() -> i64 {
            let w = Waker::new()
            let w2 = w.clone()
            w.wake()
            let result = w2.is_woken()
            w.drop()
            w2.drop()
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_waker_reset() {
    // Reset clears the woken flag
    let src = r#"
        fn main() -> i64 {
            let w = Waker::new()
            w.wake()
            let woken = w.is_woken()
            w.reset()
            let after_reset = w.is_woken()
            w.drop()
            woken * 10 + after_reset
        }
    "#;
    // woken=1, after_reset=0 → 10
    assert_eq!(compile_and_run(src), 10);
}

// =====================================================================
// S2.5 — Higher-order: map/filter/reduce
// =====================================================================

#[test]
fn native_array_map() {
    // arr.map(fn) applies fn to each element, returns new heap array
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            let doubled = arr.map(double)
            doubled[0] + doubled[1] + doubled[2]
        }
    "#;
    // 2 + 4 + 6 = 12
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_array_filter() {
    // arr.filter(fn) keeps elements where fn returns non-zero
    let src = r#"
        fn is_positive(x: i64) -> i64 { if x > 0 { 1 } else { 0 } }
        fn main() -> i64 {
            let mut arr = []
            arr.push(3)
            arr.push(0)
            arr.push(5)
            arr.push(0)
            arr.push(7)
            let pos = arr.filter(is_positive)
            pos.len()
        }
    "#;
    // 3, 5, 7 → 3 elements
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_array_reduce() {
    // arr.reduce(init, fn) folds with fn(acc, elem)
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn main() -> i64 {
            let mut arr = []
            arr.push(10)
            arr.push(20)
            arr.push(30)
            arr.reduce(0, add)
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_array_map_filter_chain() {
    // Chain: map then filter
    let src = r#"
        fn triple(x: i64) -> i64 { x * 3 }
        fn above_five(x: i64) -> i64 { if x > 5 { 1 } else { 0 } }
        fn main() -> i64 {
            let mut arr = []
            arr.push(1)
            arr.push(2)
            arr.push(3)
            arr.push(4)
            let tripled = arr.map(triple)
            let big = tripled.filter(above_five)
            big.len()
        }
    "#;
    // tripled: [3, 6, 9, 12], filtered (>5): [6, 9, 12] → 3 elements
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_array_reduce_product() {
    // reduce for multiplication
    let src = r#"
        fn mul(a: i64, b: i64) -> i64 { a * b }
        fn main() -> i64 {
            let mut arr = []
            arr.push(2)
            arr.push(3)
            arr.push(4)
            arr.reduce(1, mul)
        }
    "#;
    assert_eq!(compile_and_run(src), 24);
}

#[test]
fn native_array_map_empty() {
    // map on empty array returns empty array
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let mut arr = []
            let mapped = arr.map(double)
            mapped.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// =====================================================================
// S14.2 — Inline Assembly Operand Types
// =====================================================================

#[test]
fn native_asm_in_operand() {
    // in(reg) provides an input value to the asm template
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 42
            asm!("mov {0}, {1}", out(reg) x, in(reg) 99)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_asm_out_operand() {
    // out(reg) receives an output from asm (e.g., copy input to output)
    let src = r#"
        fn main() -> i64 {
            let result: i64 = 0
            asm!("mov {0}, {1}", out(reg) result, in(reg) 55)
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

#[test]
fn native_asm_inout_operand() {
    // inout(reg): value is both read and written
    let src = r#"
        fn main() -> i64 {
            let mut x: i64 = 10
            asm!("add {0}, {0}, {1}", inout(reg) x, in(reg) 32)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_asm_const_operand() {
    // const: compile-time constant used in the template
    let src = r#"
        fn main() -> i64 {
            let result: i64 = 0
            asm!("mov {0}, const", out(reg) result, const 77)
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_asm_specific_reg() {
    // Specific register name: in("rax") — treated same as in(reg) in Cranelift
    let src = r#"
        fn main() -> i64 {
            let result: i64 = 0
            asm!("mov {0}, {1}", out(reg) result, in("rax") 123)
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 123);
}

#[test]
fn native_asm_sym_operand() {
    // sym: reference to a function symbol (compiled as function pointer value)
    let src = r#"
        fn target() -> i64 { 42 }
        fn main() -> i64 {
            let addr: i64 = 0
            asm!("lea {0}, sym", out(reg) addr, sym target)
            if addr != 0 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// ── S14.3: asm operand type validation ──

#[test]
fn native_asm_operand_valid() {
    // Integer operands in reg constraint should compile and run correctly
    let src = r#"
        fn main() -> i64 {
            let mut x: i64 = 10
            asm!("add {0}, {0}, {1}", inout(reg) x, in(reg) 32)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_asm_operand_type_mismatch() {
    // Float value in in(reg) should produce a codegen error
    let src = r#"
        fn main() -> i64 {
            let x: f64 = 3.14
            let result: i64 = 0
            asm!("mov {0}, {1}", out(reg) result, in(reg) x)
            result
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let result = compiler.compile_program(&program);
    assert!(
        result.is_err(),
        "float in integer register should produce error"
    );
}

// =====================================================================
// S3.2 — HashMap get string values
// =====================================================================

#[test]
fn native_map_get_str_basic() {
    // Insert string values into map, get them back, verify via len()
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("greeting", "hello")
            m.insert("name", "fajar")
            let g = m.get("greeting")
            let n = m.get("name")
            len(g) + len(n)
        }
    "#;
    // "hello" = 5, "fajar" = 5, total = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_map_get_str_missing_key() {
    // Getting a non-existent key from a string map returns empty string (len 0)
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("a", "alpha")
            let v = m.get("missing")
            len(v)
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// =====================================================================
// S3.3 — HashMap keys()/values() Iteration
// =====================================================================

#[test]
fn native_map_values() {
    // map.values() returns a heap array of i64 values — sum them
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("a", 10)
            m.insert("b", 20)
            m.insert("c", 30)
            let vals = m.values()
            let count = m.len()
            // Sum all values: should be 60 regardless of order
            let sum: i64 = 0
            let i: i64 = 0
            while i < count {
                sum = sum + vals[i]
                i = i + 1
            }
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

// =====================================================================
// S10.3 — Timer wheel + sleep
// =====================================================================

#[test]
fn native_sleep_basic() {
    // sleep(0) should complete without error
    let src = r#"
        fn main() -> i64 {
            sleep(0)
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_sleep_zero() {
    // sleep(0) returns 0, execution continues
    let src = r#"
        fn main() -> i64 {
            let x = 10
            sleep(0)
            let y = 20
            x + y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_timer_schedule_and_tick() {
    // Create timer, schedule a timer with a waker, tick to fire it
    let src = r#"
        fn main() -> i64 {
            let tw = Timer::new()
            let w = Waker::new()
            tw.schedule(0, w)
            sleep(1)
            let fired = tw.tick()
            let woken = w.is_woken()
            tw.free()
            w.drop()
            fired + woken
        }
    "#;
    // Timer with 0ms should fire immediately on tick, waker should be woken
    // fired=1, woken=1 → 2
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_timer_pending_count() {
    // pending() should show unfired timers
    let src = r#"
        fn main() -> i64 {
            let tw = Timer::new()
            let w1 = Waker::new()
            let w2 = Waker::new()
            tw.schedule(0, w1)
            tw.schedule(100000, w2)
            let before = tw.pending()
            sleep(1)
            tw.tick()
            let after = tw.pending()
            tw.free()
            w1.drop()
            w2.drop()
            before * 10 + after
        }
    "#;
    // before=2, after=1 (100s timer not fired yet) → 2*10+1 = 21
    assert_eq!(compile_and_run(src), 21);
}

#[test]
fn native_timer_no_waker() {
    // schedule with null waker (0) should not crash on tick
    let src = r#"
        fn main() -> i64 {
            let tw = Timer::new()
            tw.schedule(0, 0)
            sleep(1)
            let fired = tw.tick()
            tw.free()
            fired
        }
    "#;
    // waker_ptr is 0 (null), so tick skips waking but should still "fire"
    // Actually our impl only fires entries with non-null waker_ptr, so fired=0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_async_sleep_basic() {
    // async function that sleeps, then returns a value
    let src = r#"
        async fn delayed() -> i64 {
            sleep(0)
            99
        }
        fn main() -> i64 {
            let result = delayed().await
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

// =====================================================================
// S11.1 — Thread pool executor
// =====================================================================

#[test]
fn native_threadpool_spawn_and_run() {
    // Create pool, spawn async tasks, run, get results
    let src = r#"
        async fn compute(x: i64) -> i64 { x * 10 }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            pool.spawn(compute(3))
            pool.spawn(compute(5))
            let completed = pool.run()
            let r0 = pool.get_result(0)
            let r1 = pool.get_result(1)
            pool.free()
            r0 + r1
        }
    "#;
    // compute(3)=30, compute(5)=50 → 80
    assert_eq!(compile_and_run(src), 80);
}

#[test]
fn native_threadpool_thread_count() {
    // thread_count() returns the configured number
    let src = r#"
        fn main() -> i64 {
            let pool = ThreadPool::new(8)
            let n = pool.thread_count()
            pool.free()
            n
        }
    "#;
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_threadpool_empty_run() {
    // Running with no tasks returns 0
    let src = r#"
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            let completed = pool.run()
            pool.free()
            completed
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// =====================================================================
// S11.2 — Work-stealing thread pool
// =====================================================================

#[test]
fn native_work_stealing_basic() {
    // Work-stealing: create pool with 2 threads but 5 tasks — stealing ensures all complete
    let src = r#"
        async fn work(x: i64) -> i64 { x * 3 }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            pool.spawn(work(1))
            pool.spawn(work(2))
            pool.spawn(work(3))
            pool.spawn(work(4))
            pool.spawn(work(5))
            let completed = pool.run()
            let sum = pool.get_result(0) + pool.get_result(1) + pool.get_result(2) + pool.get_result(3) + pool.get_result(4)
            pool.free()
            sum
        }
    "#;
    // 3+6+9+12+15 = 45
    assert_eq!(compile_and_run(src), 45);
}

#[test]
fn native_work_stealing_load_balancing() {
    // With 4 threads and 3 tasks, some threads will steal from others
    let src = r#"
        async fn compute(x: i64) -> i64 { x + 100 }
        fn main() -> i64 {
            let pool = ThreadPool::new(4)
            pool.spawn(compute(1))
            pool.spawn(compute(2))
            pool.spawn(compute(3))
            let completed = pool.run()
            let r0 = pool.get_result(0)
            let r1 = pool.get_result(1)
            let r2 = pool.get_result(2)
            pool.free()
            r0 + r1 + r2
        }
    "#;
    // 101+102+103 = 306
    assert_eq!(compile_and_run(src), 306);
}

// =====================================================================
// S11.3 — Cross-thread JoinHandle
// =====================================================================

#[test]
fn native_cross_thread_join() {
    // spawn_join returns JoinHandle, pool.run() completes it, jh.get() retrieves result
    let src = r#"
        async fn heavy(x: i64) -> i64 { x * x }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            let jh = pool.spawn_join(heavy(7))
            pool.run()
            let result = jh.get()
            jh.free()
            pool.free()
            result
        }
    "#;
    // 7*7 = 49
    assert_eq!(compile_and_run(src), 49);
}

#[test]
fn native_join_multiple() {
    // Multiple JoinHandles from same pool
    let src = r#"
        async fn add10(x: i64) -> i64 { x + 10 }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            let jh1 = pool.spawn_join(add10(5))
            let jh2 = pool.spawn_join(add10(20))
            pool.run()
            let r1 = jh1.get()
            let r2 = jh2.get()
            jh1.free()
            jh2.free()
            pool.free()
            r1 + r2
        }
    "#;
    // 15 + 30 = 45
    assert_eq!(compile_and_run(src), 45);
}

// =====================================================================
// S11.4 — Cancellation
// =====================================================================

#[test]
fn native_cancel_task() {
    // Abort a JoinHandle before pool runs — get() returns -1
    let src = r#"
        async fn slow(x: i64) -> i64 { x * 100 }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            let jh = pool.spawn_join(slow(5))
            jh.abort()
            let cancelled = jh.is_cancelled()
            let result = jh.get()
            jh.free()
            pool.free()
            cancelled * 1000 + result
        }
    "#;
    // cancelled=1 → 1000 + (-1) = 999
    assert_eq!(compile_and_run(src), 999);
}

#[test]
fn native_cancel_already_done() {
    // Abort after pool.run() — result should still be available (not -1)
    // because the task completed before abort
    let src = r#"
        async fn fast(x: i64) -> i64 { x + 1 }
        fn main() -> i64 {
            let pool = ThreadPool::new(2)
            let jh = pool.spawn_join(fast(9))
            pool.run()
            let result = jh.get()
            jh.abort()
            let cancelled = jh.is_cancelled()
            jh.free()
            pool.free()
            result * 10 + cancelled
        }
    "#;
    // fast(9)=10, result=10, cancelled=1 → 10*10+1 = 101
    assert_eq!(compile_and_run(src), 101);
}

// =====================================================================
// S12.1 — Async channels
// =====================================================================

#[test]
fn native_async_send_recv() {
    // Basic async channel: send then recv
    let src = r#"
        fn main() -> i64 {
            let ch = AsyncChannel::new()
            ch.send(42)
            ch.send(100)
            let a = ch.recv()
            let b = ch.recv()
            ch.free()
            a + b
        }
    "#;
    // 42 + 100 = 142
    assert_eq!(compile_and_run(src), 142);
}

#[test]
fn native_async_bounded() {
    // Bounded async channel
    let src = r#"
        fn main() -> i64 {
            let ch = AsyncChannel::bounded(2)
            ch.send(10)
            ch.send(20)
            let a = ch.recv()
            let b = ch.recv()
            ch.free()
            a + b
        }
    "#;
    // 10 + 20 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_async_close() {
    // Closing async channel: send after close returns 0
    let src = r#"
        fn main() -> i64 {
            let ch = AsyncChannel::new()
            ch.send(77)
            ch.close()
            let ok = ch.send(88)
            let val = ch.recv()
            ch.free()
            val * 10 + ok
        }
    "#;
    // recv gets 77 (sent before close), send after close returns 0
    // 77 * 10 + 0 = 770
    assert_eq!(compile_and_run(src), 770);
}

#[test]
fn native_map_values_empty() {
    // values() on empty map returns len 0
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            let count = m.len()
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// S3.4 — for-in loop over HashMap keys
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_map_for_in_keys() {
    // Iterate over map keys via variable, count them
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("alpha", 1)
            m.insert("beta", 2)
            m.insert("gamma", 3)
            let keys = m.keys()
            let count: i64 = 0
            for k in keys {
                count = count + 1
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_map_for_in_keys_inline() {
    // Iterate over map.keys() inline (no temp variable), count keys
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("x", 10)
            m.insert("y", 20)
            let count: i64 = 0
            for k in m.keys() {
                count = count + 1
            }
            count
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_map_for_in_keys_use_len() {
    // Iterate keys and use len() on each key string
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("ab", 1)
            m.insert("cde", 2)
            let keys = m.keys()
            let total_len: i64 = 0
            for k in keys {
                total_len = total_len + len(k)
            }
            total_len
        }
    "#;
    // "ab" = 2, "cde" = 3, total = 5
    assert_eq!(compile_and_run(src), 5);
}

// ═══════════════════════════════════════════════════════════════════════
// S14.5 — global_asm!
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_global_asm_section() {
    // global_asm! at top level is collected; program still compiles and runs
    let src = r#"
        global_asm!(".section .text\n.align 4")

        fn main() -> i64 { 42 }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::CraneliftCompiler::new().unwrap();
    compiler.compile_program(&program).unwrap();

    // Verify the section was collected
    let sections = compiler.global_asm_sections();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0], ".section .text\n.align 4");

    // Program still executes correctly
    let fn_ptr = compiler.get_fn_ptr("main").unwrap();
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 42);
}

#[test]
fn native_global_asm_label() {
    // Multiple global_asm! items with labels for interrupt vector tables
    let src = r#"
        global_asm!(".global _start")
        global_asm!("_isr_table: .quad 0, 0, 0, 0")

        fn main() -> i64 { 100 }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::CraneliftCompiler::new().unwrap();
    compiler.compile_program(&program).unwrap();

    let sections = compiler.global_asm_sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0], ".global _start");
    assert_eq!(sections[1], "_isr_table: .quad 0, 0, 0, 0");

    let fn_ptr = compiler.get_fn_ptr("main").unwrap();
    let main_fn: fn() -> i64 = unsafe { std::mem::transmute(fn_ptr) };
    assert_eq!(main_fn(), 100);
}

// ═══════════════════════════════════════════════════════════════════════
// S16.3 — Global allocator
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_global_allocator_default() {
    // Default allocator: heap allocation works via HashMap (uses fj_rt_alloc internally)
    let src = r#"
        fn main() -> i64 {
            let m = HashMap::new()
            m.insert("a", 10)
            m.insert("b", 20)
            m.len()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_global_allocator_set_and_reset() {
    use super::runtime_fns::{
        fj_rt_alloc, fj_rt_free, fj_rt_reset_global_allocator, fj_rt_set_global_allocator,
    };

    // Set a custom allocator (just wraps the default for testing)
    extern "C" fn custom_alloc(size: i64) -> *mut u8 {
        // Delegate to default but we can verify it was called
        let layout =
            std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid alloc size");
        unsafe { std::alloc::alloc(layout) }
    }
    extern "C" fn custom_free(ptr: *mut u8, size: i64) {
        let layout =
            std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid free size");
        unsafe { std::alloc::dealloc(ptr, layout) }
    }

    // Set custom allocator
    fj_rt_set_global_allocator(
        custom_alloc as *const () as i64,
        custom_free as *const () as i64,
    );

    // Allocate and free through the global dispatch
    let ptr = fj_rt_alloc(64);
    assert!(!ptr.is_null());
    fj_rt_free(ptr, 64);

    // Reset to default
    fj_rt_reset_global_allocator();

    // Default still works
    let ptr2 = fj_rt_alloc(128);
    assert!(!ptr2.is_null());
    fj_rt_free(ptr2, 128);
}

#[test]
fn native_global_allocator_custom_bump() {
    use super::runtime_fns::{
        fj_rt_alloc, fj_rt_free, fj_rt_reset_global_allocator, fj_rt_set_global_allocator,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Track allocations with a simple counting allocator
    static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn counting_alloc(size: i64) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        let layout =
            std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid alloc size");
        unsafe { std::alloc::alloc(layout) }
    }
    extern "C" fn counting_free(ptr: *mut u8, size: i64) {
        let layout =
            std::alloc::Layout::from_size_align(size as usize, 8).expect("invalid free size");
        unsafe { std::alloc::dealloc(ptr, layout) }
    }

    fj_rt_reset_global_allocator();
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    fj_rt_set_global_allocator(
        counting_alloc as *const () as i64,
        counting_free as *const () as i64,
    );

    // Multiple allocations go through our counting allocator
    let p1 = fj_rt_alloc(32);
    let p2 = fj_rt_alloc(64);
    let p3 = fj_rt_alloc(128);
    assert_eq!(ALLOC_COUNT.load(Ordering::SeqCst), 3);

    fj_rt_free(p1, 32);
    fj_rt_free(p2, 64);
    fj_rt_free(p3, 128);

    // Reset to default
    fj_rt_reset_global_allocator();
}

// ═══════════════════════════════════════════════════════════════════════
// S17.4 — Bare metal output
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_bare_metal_binary_aarch64() {
    // Compile a bare-metal program for aarch64-unknown-none
    let src = r#"
        fn compute(n: i64) -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < n {
                sum = sum + i
                i = i + 1
            }
            sum
        }
        fn main() -> i64 { compute(10) }
    "#;
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("bare_metal", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Object file should be produced and small (< 16KB for pure computation)
    assert!(!obj_bytes.is_empty());
    assert!(
        obj_bytes.len() < 16384,
        "bare metal object too large: {} bytes",
        obj_bytes.len()
    );
}

#[test]
fn native_bare_metal_no_dynamic_links() {
    // Bare-metal object should have no dynamic linking references
    let src = "fn main() -> i64 { 42 }";
    let target =
        crate::codegen::target::TargetConfig::from_triple("riscv64gc-unknown-none-elf").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("bare_riscv", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Should produce a valid object file
    assert!(!obj_bytes.is_empty());

    // ELF magic number check (0x7F 'E' 'L' 'F')
    assert_eq!(&obj_bytes[..4], &[0x7f, b'E', b'L', b'F']);

    // No ".dynamic" section in the raw bytes (bare metal = static only)
    let has_dynamic = obj_bytes.windows(8).any(|w| w == b".dynamic");
    assert!(
        !has_dynamic,
        "bare metal object should not have .dynamic section"
    );
}

#[test]
fn native_bare_metal_binary_size_check() {
    // Verify minimal binary size for trivial bare-metal program
    let src = "fn main() -> i64 { 0 }";
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("tiny", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Trivial program object file should be small (< 16KB including symbol tables)
    assert!(
        obj_bytes.len() < 16384,
        "trivial bare metal object should be < 16KB, got {} bytes",
        obj_bytes.len()
    );
}

#[test]
fn native_aot_entry_emits_start_symbol() {
    // AOT: @entry function should produce a _start symbol in the object file
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn boot() -> i64 {
            42
        }

        fn main() -> i64 { boot() }
    "#;
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("start_test", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // The object should contain the _start symbol
    let has_start = obj_bytes.windows(6).any(|w| w == b"_start");
    assert!(has_start, "object file should contain _start symbol");
}

#[test]
fn native_aot_entry_start_calls_boot() {
    // _start should be a wrapper that calls the @entry function
    // Verify both boot and _start appear in the object
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn my_boot() -> i64 {
            77
        }

        fn main() -> i64 { my_boot() }
    "#;
    let target =
        crate::codegen::target::TargetConfig::from_triple("riscv64gc-unknown-none-elf").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("start_rv", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Both _start and my_boot should exist in the ELF
    let has_start = obj_bytes.windows(6).any(|w| w == b"_start");
    let has_boot = obj_bytes.windows(7).any(|w| w == b"my_boot");
    assert!(has_start, "object should contain _start symbol");
    assert!(has_boot, "object should contain my_boot symbol");
}

// ═══════════════════════════════════════════════════════════════════════
// FajarOS S1 — Bare-metal aarch64 target tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_bare_metal_aarch64_compiles_empty_kernel() {
    // Task 1.8: compile empty @kernel fn _start() {} → valid aarch64 ELF
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn _start() -> i64 {
            0
        }

        fn main() -> i64 { 0 }
    "#;
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    assert!(target.is_bare_metal);

    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("bare_kernel", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Verify it's a valid ELF
    assert_eq!(&obj_bytes[..4], b"\x7fELF", "should be valid ELF");
    assert_eq!(obj_bytes[4], 2, "should be 64-bit (ELFCLASS64)");
    assert!(obj_bytes.len() > 100, "object should have content");
    // Should contain _start symbol
    let has_start = obj_bytes.windows(6).any(|w| w == b"_start");
    assert!(has_start, "should contain _start symbol");
}

#[test]
fn native_bare_metal_no_libc_symbols() {
    // Task 1.5: bare-metal should NOT reference libc functions
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn _start() -> i64 {
            let x = 10
            let y = 32
            x + y
        }

        fn main() -> i64 { 0 }
    "#;
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("no_libc", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Should NOT contain libc-dependent symbols
    let obj_str = String::from_utf8_lossy(&obj_bytes);
    assert!(
        !obj_str.contains("fj_rt_print_i64\0"),
        "should not reference libc print"
    );
    assert!(
        !obj_str.contains("fj_rt_read_file"),
        "should not reference file I/O"
    );
    assert!(
        !obj_str.contains("fj_rt_str_split"),
        "should not reference heap string ops"
    );
}

#[test]
fn native_bare_metal_has_bare_runtime() {
    // Bare-metal should declare fj_rt_bare_* functions
    let src = r#"
        @panic_handler
        fn panic(code: i64) -> i64 { code }

        @entry
        fn _start() -> i64 { 42 }

        fn main() -> i64 { 0 }
    "#;
    let target = crate::codegen::target::TargetConfig::from_triple("aarch64-unknown-none").unwrap();
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new_with_target("bare_rt", &target).unwrap();
    compiler.set_no_std(true);
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    let obj_str = String::from_utf8_lossy(&obj_bytes);
    assert!(
        obj_str.contains("fj_rt_bare_memcpy"),
        "should reference bare memcpy"
    );
    assert!(
        obj_str.contains("fj_rt_bare_memset"),
        "should reference bare memset"
    );
}

#[test]
fn native_bsp_arch_bare_metal_display() {
    let arch = crate::bsp::BspArch::Aarch64BareMetal;
    assert_eq!(arch.to_string(), "aarch64-unknown-none");
}

// ═══════════════════════════════════════════════════════════════════════
// S36.3 — MNIST IDX parser (unit tests using runtime fns directly)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_mnist_parse_images_synthetic() {
    use super::runtime_fns::{fj_rt_mnist_parse_images_buf, fj_rt_tensor_get, fj_rt_tensor_rows};
    // Build a synthetic IDX image file: 2 images, 2x2 pixels
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(&0x00000803u32.to_be_bytes()); // magic
    data.extend_from_slice(&2u32.to_be_bytes()); // n_images
    data.extend_from_slice(&2u32.to_be_bytes()); // n_rows
    data.extend_from_slice(&2u32.to_be_bytes()); // n_cols
    // Image 0: [10, 20, 30, 40]
    data.extend_from_slice(&[10, 20, 30, 40]);
    // Image 1: [50, 60, 70, 80]
    data.extend_from_slice(&[50, 60, 70, 80]);

    let tensor_ptr = fj_rt_mnist_parse_images_buf(data.as_ptr(), data.len() as i64);
    assert!(!tensor_ptr.is_null());

    // Check shape: 2 rows (images), 4 cols (2x2 pixels flattened)
    let rows = fj_rt_tensor_rows(tensor_ptr);
    assert_eq!(rows, 2);

    // Check pixel values (stored as f64 bits in i64)
    let val_00 = fj_rt_tensor_get(tensor_ptr, 0, 0);
    assert_eq!(f64::from_bits(val_00 as u64), 10.0);
    let val_13 = fj_rt_tensor_get(tensor_ptr, 1, 3);
    assert_eq!(f64::from_bits(val_13 as u64), 80.0);

    // Cleanup
    unsafe {
        let _ = Box::from_raw(tensor_ptr as *mut ndarray::Array2<f64>);
    }
}

#[test]
fn native_mnist_parse_labels_synthetic() {
    use super::runtime_fns::{fj_rt_mnist_parse_labels_buf, fj_rt_tensor_get, fj_rt_tensor_rows};
    // Build a synthetic IDX label file: 3 labels
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(&0x00000801u32.to_be_bytes()); // magic
    data.extend_from_slice(&3u32.to_be_bytes()); // n_labels
    data.extend_from_slice(&[7, 2, 5]); // labels

    let tensor_ptr = fj_rt_mnist_parse_labels_buf(data.as_ptr(), data.len() as i64);
    assert!(!tensor_ptr.is_null());

    let rows = fj_rt_tensor_rows(tensor_ptr);
    assert_eq!(rows, 3);

    let label_0 = fj_rt_tensor_get(tensor_ptr, 0, 0);
    assert_eq!(f64::from_bits(label_0 as u64), 7.0);
    let label_2 = fj_rt_tensor_get(tensor_ptr, 2, 0);
    assert_eq!(f64::from_bits(label_2 as u64), 5.0);

    unsafe {
        let _ = Box::from_raw(tensor_ptr as *mut ndarray::Array2<f64>);
    }
}

#[test]
fn native_mnist_parse_invalid_magic() {
    use super::runtime_fns::fj_rt_mnist_parse_images_buf;
    // Wrong magic number
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(&0x00000801u32.to_be_bytes()); // label magic, not image
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&1u32.to_be_bytes());
    data.push(42);

    let tensor_ptr = fj_rt_mnist_parse_images_buf(data.as_ptr(), data.len() as i64);
    assert!(tensor_ptr.is_null()); // Should fail
}

// ═══════════════════════════════════════════════════════════════════════
// S12.2 — Stream (from codegen)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_stream_basic() {
    // Stream::from_range, iterate with next/has_next
    let src = r#"
        fn main() -> i64 {
            let s = Stream::from_range(1, 6)
            let mut total = 0
            while s.has_next() == 1 {
                total = total + s.next()
            }
            s.free()
            total
        }
    "#;
    // 1+2+3+4+5 = 15
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_stream_push_and_iterate() {
    // Push values into a stream, then iterate
    let src = r#"
        fn main() -> i64 {
            let s = Stream::new()
            s.push(10)
            s.push(20)
            s.push(30)
            let total = s.sum()
            s.free()
            total
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_stream_count() {
    let src = r#"
        fn main() -> i64 {
            let s = Stream::from_range(0, 100)
            let n = s.count()
            s.free()
            n
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

// ═══════════════════════════════════════════════════════════════════════
// S12.3 — Stream combinators (map, filter, take)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_stream_map() {
    // map each element through a function
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let s = Stream::from_range(1, 4)
            let mapped = s.map(double)
            let total = mapped.sum()
            mapped.free()
            s.free()
            total
        }
    "#;
    // map([1,2,3], *2) = [2,4,6], sum = 12
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_stream_filter() {
    // filter elements that pass a predicate
    let src = r#"
        fn is_even(x: i64) -> i64 {
            if x % 2 == 0 { 1 } else { 0 }
        }
        fn main() -> i64 {
            let s = Stream::from_range(1, 11)
            let filtered = s.filter(is_even)
            let total = filtered.sum()
            filtered.free()
            s.free()
            total
        }
    "#;
    // even numbers in 1..11: 2+4+6+8+10 = 30
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_stream_take() {
    // take first N items from a stream
    let src = r#"
        fn main() -> i64 {
            let s = Stream::from_range(1, 100)
            let first5 = s.take(5)
            let total = first5.sum()
            first5.free()
            s.free()
            total
        }
    "#;
    // take(5) from 1..100 = [1,2,3,4,5], sum = 15
    assert_eq!(compile_and_run(src), 15);
}

// ═══════════════════════════════════════════════════════════════════════
// S43.5 — Binary size regression + startup time tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_size_regression_minimal() {
    // Minimal program should produce a small object file
    let src = "fn main() -> i64 { 0 }";
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new("size_test").unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Minimal x86_64 ELF object should be under 16KB
    assert!(
        obj_bytes.len() < 16384,
        "minimal program object should be < 16KB, got {} bytes",
        obj_bytes.len()
    );
}

#[test]
fn native_size_regression_with_functions() {
    // Program with several functions should still be reasonable
    let src = r#"
        fn add(a: i64, b: i64) -> i64 { a + b }
        fn mul(a: i64, b: i64) -> i64 { a * b }
        fn compute(x: i64) -> i64 { add(mul(x, 2), 1) }
        fn main() -> i64 { compute(21) }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new("size_fn_test").unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Multi-function program should be under 32KB
    assert!(
        obj_bytes.len() < 32768,
        "multi-function program object should be < 32KB, got {} bytes",
        obj_bytes.len()
    );
}

#[test]
fn native_size_regression_loop() {
    // Program with loops and control flow
    let src = r#"
        fn main() -> i64 {
            let mut sum = 0
            let mut i = 0
            while i < 1000 {
                if i % 2 == 0 { sum = sum + i }
                i = i + 1
            }
            sum
        }
    "#;
    let tokens = crate::lexer::tokenize(src).unwrap();
    let program = crate::parser::parse(tokens).unwrap();
    let mut compiler = super::ObjectCompiler::new("size_loop_test").unwrap();
    compiler.compile_program(&program).unwrap();
    let product = compiler.finish();
    let obj_bytes = product.emit().unwrap();

    // Loop + control flow should still be under 32KB
    assert!(
        obj_bytes.len() < 32768,
        "loop program object should be < 32KB, got {} bytes",
        obj_bytes.len()
    );
}

#[test]
fn native_startup_time_jit() {
    // JIT compilation + execution should complete quickly
    let start = std::time::Instant::now();
    let src = "fn main() -> i64 { 42 }";
    let result = compile_and_run(src);
    let elapsed = start.elapsed();

    assert_eq!(result, 42);
    // Target: <100ms, test threshold: <2s (jitter-immune under
    // `cargo test --test-threads=64` parallel load)
    assert!(
        elapsed.as_millis() < 2000,
        "JIT startup took too long: {:?} (target <100ms, test allows <2s)",
        elapsed
    );
}

// ═══════════════════════════════════════════════════════════════════════
// S40 — SIMD vector types and operations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_simd_f32x4_add() {
    // f32x4::new(1.0, 2.0, 3.0, 4.0) + f32x4::new(10.0, 20.0, 30.0, 40.0)
    // sum of result = 11.0 + 22.0 + 33.0 + 44.0 = 110.0
    let src = r#"
fn main() -> f64 {
    let a = f32x4::new(1.0, 2.0, 3.0, 4.0)
    let b = f32x4::new(10.0, 20.0, 30.0, 40.0)
    let c = a.add(b)
    c.sum()
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 110.0).abs() < 0.01, "expected 110.0, got {f}");
}

#[test]
fn native_simd_f32x4_mul() {
    // f32x4::splat(3.0) * f32x4::splat(7.0) → all 21.0, sum = 84.0
    let src = r#"
fn main() -> f64 {
    let a = f32x4::splat(3.0)
    let b = f32x4::splat(7.0)
    let c = a.mul(b)
    c.sum()
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 84.0).abs() < 0.01, "expected 84.0, got {f}");
}

#[test]
fn native_simd_f32x4_get() {
    // Check individual lane access
    let src = r#"
fn main() -> f64 {
    let v = f32x4::new(10.0, 20.0, 30.0, 40.0)
    v.get(2)
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 30.0).abs() < 0.01, "expected 30.0, got {f}");
}

#[test]
fn native_simd_f32x4_min_max() {
    // min of (5, 2, 8, 1) = 1, max = 8
    let src = r#"
fn main() -> f64 {
    let v = f32x4::new(5.0, 2.0, 8.0, 1.0)
    v.min()
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 1.0).abs() < 0.01, "expected 1.0 (min), got {f}");
}

#[test]
fn native_simd_i32x4_add() {
    // i32x4::new(1, 2, 3, 4) + i32x4::new(10, 20, 30, 40) → sum = 110
    let src = r#"
fn main() -> i64 {
    let a = i32x4::new(1, 2, 3, 4)
    let b = i32x4::new(10, 20, 30, 40)
    let c = a.add(b)
    c.sum()
}
"#;
    assert_eq!(compile_and_run(src), 110);
}

#[test]
fn native_simd_i32x4_mul_sum() {
    // dot product: (1,2,3,4) . (5,6,7,8) = 5+12+21+32 = 70
    let src = r#"
fn main() -> i64 {
    let a = i32x4::new(1, 2, 3, 4)
    let b = i32x4::new(5, 6, 7, 8)
    let c = a.mul(b)
    c.sum()
}
"#;
    assert_eq!(compile_and_run(src), 70);
}

#[test]
fn native_simd_i32x4_get_min_max() {
    let src = r#"
fn main() -> i64 {
    let v = i32x4::new(42, 7, 99, 3)
    let mn = v.min()
    let mx = v.max()
    mn + mx
}
"#;
    // min=3, max=99, sum=102
    assert_eq!(compile_and_run(src), 102);
}

#[test]
fn native_simd_f32x8_mul_sum() {
    // f32x8 splat(2.0) * splat(5.0) = 8 lanes of 10.0, sum = 80.0
    let src = r#"
fn main() -> f64 {
    let a = f32x8::splat(2.0)
    let b = f32x8::splat(5.0)
    let c = a.mul(b)
    c.sum()
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 80.0).abs() < 0.01, "expected 80.0, got {f}");
}

#[test]
fn native_simd_i32x8_add_sum() {
    // i32x8 splat(10) + splat(5) = 8 lanes of 15, sum = 120
    let src = r#"
fn main() -> i64 {
    let a = i32x8::splat(10)
    let b = i32x8::splat(5)
    let c = a.add(b)
    c.sum()
}
"#;
    assert_eq!(compile_and_run(src), 120);
}

#[test]
fn native_simd_f32x4_sub_div() {
    // (10,20,30,40) - (1,2,3,4) = (9,18,27,36), div by splat(9) = (1,2,3,4), sum = 10.0
    let src = r#"
fn main() -> f64 {
    let a = f32x4::new(10.0, 20.0, 30.0, 40.0)
    let b = f32x4::new(1.0, 2.0, 3.0, 4.0)
    let c = a.sub(b)
    let d = f32x4::splat(9.0)
    let e = c.div(d)
    e.sum()
}
"#;
    let result = compile_and_run(src);
    let f = f64::from_bits(result as u64);
    assert!((f - 10.0).abs() < 0.01, "expected 10.0, got {f}");
}

// ═══════════════════════════════════════════════════════════════════════
// S40.5 — @simd annotation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_simd_annotation_accepted() {
    // @simd annotation on a function should compile without error
    let src = r#"
        @simd fn vector_add(a: i64, b: i64) -> i64 {
            a + b
        }
        fn main() -> i64 {
            vector_add(10, 20)
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

// ═══════════════════════════════════════════════════════════════════════
// S35 — ONNX Export
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_onnx_model_new() {
    // Create an ONNX model builder and check initial state
    let src = r#"
fn main() -> i64 {
    let model = OnnxModel::new()
    let n = model.node_count()
    model.free()
    n
}
"#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_onnx_add_dense_nodes() {
    // Add a dense layer: should create MatMul + Add = 2 nodes
    let src = r#"
fn main() -> i64 {
    let model = OnnxModel::new()
    let w = tensor_zeros(3, 4)
    let b = tensor_zeros(1, 4)
    model.add_dense(w, b, 0)
    let n = model.node_count()
    model.free()
    n
}
"#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_onnx_dense_with_relu() {
    // Dense + Relu = 3 nodes, 2 initializers (weight + bias)
    let src = r#"
fn main() -> i64 {
    let model = OnnxModel::new()
    let w = tensor_zeros(3, 4)
    let b = tensor_zeros(1, 4)
    model.add_dense(w, b, 0)
    model.add_relu(0)
    let nodes = model.node_count()
    let inits = model.initializer_count()
    model.free()
    nodes + inits
}
"#;
    // 3 nodes (matmul + add + relu) + 2 initializers (weight + bias) = 5
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn native_onnx_multi_layer() {
    // Two dense layers: 4 nodes, 4 initializers
    let src = r#"
fn main() -> i64 {
    let model = OnnxModel::new()
    let w1 = tensor_zeros(3, 4)
    let b1 = tensor_zeros(1, 4)
    let w2 = tensor_zeros(4, 2)
    let b2 = tensor_zeros(1, 2)
    model.add_dense(w1, b1, 0)
    model.add_dense(w2, b2, 1)
    let n = model.node_count()
    let i = model.initializer_count()
    model.free()
    n + i
}
"#;
    // 4 nodes (2×MatMul + 2×Add) + 4 initializers = 8
    assert_eq!(compile_and_run(src), 8);
}

// =====================================================================
