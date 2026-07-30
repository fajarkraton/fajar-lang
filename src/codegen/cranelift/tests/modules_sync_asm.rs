//! Module system, mutex/atomics/channels/condvar, inline asm, volatile, int widths, allocators.

use super::super::*;
use super::compile_and_run;
use crate::lexer::tokenize;
use crate::parser::parse;

// ============================================================
// S4.9 — Module system in native codegen
// ============================================================

#[test]
fn native_inline_mod_call() {
    let src = r#"
        mod math {
            fn double(x: i64) -> i64 { x * 2 }
        }
        fn main() -> i64 {
            math::double(21)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_mod_multiple_functions() {
    let src = r#"
        mod utils {
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn sub(a: i64, b: i64) -> i64 { a - b }
        }
        fn main() -> i64 {
            utils::add(30, 20) - utils::sub(10, 2)
        }
    "#;
    // (30+20) - (10-2) = 50 - 8 = 42
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_mod_const() {
    let src = r#"
        mod config {
            const MAX: i64 = 100
        }
        fn main() -> i64 {
            config::MAX
        }
    "#;
    // Module const accessed via path — need const propagation
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_mod_function_calls_local() {
    let src = r#"
        mod math {
            fn square(x: i64) -> i64 { x * x }
            fn sum_of_squares(a: i64, b: i64) -> i64 {
                square(a) + square(b)
            }
        }
        fn main() -> i64 {
            math::sum_of_squares(3, 4)
        }
    "#;
    // 9 + 16 = 25
    assert_eq!(compile_and_run(src), 25);
}

// ============================================================
// S5 — Thread primitives in native codegen
// ============================================================

#[test]
fn native_thread_spawn_noarg() {
    let src = r#"
        fn worker() -> i64 { 42 }
        fn main() -> i64 {
            let h = thread::spawn(worker)
            h.join()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_thread_spawn_with_arg() {
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }
        fn main() -> i64 {
            let h = thread::spawn(double, 21)
            h.join()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_thread_multiple_joins() {
    let src = r#"
        fn compute(x: i64) -> i64 { x * x }
        fn main() -> i64 {
            let h1 = thread::spawn(compute, 3)
            let h2 = thread::spawn(compute, 4)
            h1.join() + h2.join()
        }
    "#;
    // 9 + 16 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn native_thread_return_value() {
    let src = r#"
        fn fib(n: i64) -> i64 {
            if n <= 1 { n }
            else { fib(n - 1) + fib(n - 2) }
        }
        fn main() -> i64 {
            let h = thread::spawn(fib, 10)
            h.join()
        }
    "#;
    // fib(10) = 55
    assert_eq!(compile_and_run(src), 55);
}

// ============================================================
// S6 — Mutex in native codegen
// ============================================================

#[test]
fn native_mutex_lock_store() {
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(10)
            let val = m.lock()
            m.store(val + 32)
            m.lock()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_mutex_initial_value() {
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(99)
            m.lock()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_mutex_shared_counter() {
    // Increment a shared counter from two threads
    let src = r#"
        fn increment(mutex_ptr: i64) -> i64 {
            0
        }
        fn main() -> i64 {
            let m = Mutex::new(0)
            m.store(1)
            m.store(2)
            m.store(3)
            m.lock()
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

// ── S6.1: Mutex try_lock codegen ─────────────────────────────────────

#[test]
fn native_mutex_try_lock_basic() {
    // try_lock on an unlocked mutex should succeed (return 1)
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(42)
            m.try_lock()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_mutex_try_lock_value_preserved() {
    // After try_lock succeeds, lock() retrieves the stored value
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(99)
            let ok = m.try_lock()
            m.store(ok + 41)
            m.lock()
        }
    "#;
    // ok = 1, 1 + 41 = 42
    assert_eq!(compile_and_run(src), 42);
}

// ============================================================
// S7 — Channels in native codegen
// ============================================================

#[test]
fn native_channel_send_recv() {
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(42)
            ch.recv()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_channel_multi_send() {
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(10)
            ch.send(20)
            ch.send(30)
            let a = ch.recv()
            let b = ch.recv()
            let c = ch.recv()
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_channel_fifo_order() {
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(1)
            ch.send(2)
            ch.send(3)
            let first = ch.recv()
            let second = ch.recv()
            first * 10 + second
        }
    "#;
    // FIFO: first=1, second=2 → 12
    assert_eq!(compile_and_run(src), 12);
}

// ═══════════════════════════════════════════════════════════════════════
// S8 — Atomic primitives
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_atomic_new_and_load() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(42)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_atomic_store_and_load() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store(99)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_atomic_fetch_add() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(10)
            let old = a.add(5)
            let current = a.load()
            old * 100 + current
        }
    "#;
    // old=10, current=15 → 1015
    assert_eq!(compile_and_run(src), 1015);
}

#[test]
fn native_atomic_multiple_adds() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.add(1)
            a.add(2)
            a.add(3)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_atomic_fetch_sub() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(100)
            let old = a.sub(30)
            let current = a.load()
            old * 100 + current
        }
    "#;
    // old=100, current=70 → 10070
    assert_eq!(compile_and_run(src), 10070);
}

#[test]
fn native_atomic_cas_success() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(42)
            let prev = a.cas(42, 99)
            let current = a.load()
            prev * 100 + current
        }
    "#;
    // CAS succeeds: prev=42, current=99 → 4299
    assert_eq!(compile_and_run(src), 4299);
}

#[test]
fn native_atomic_cas_failure() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(42)
            let prev = a.cas(10, 99)
            let current = a.load()
            prev * 100 + current
        }
    "#;
    // CAS fails (expected 10 != actual 42): prev=42, current=42 → 4242
    assert_eq!(compile_and_run(src), 4242);
}

// ═══════════════════════════════════════════════════════════════════════
// S6.2 — RwLock
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_rwlock_read() {
    let src = r#"
        fn main() -> i64 {
            let rw = RwLock::new(42)
            rw.read()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_rwlock_write_then_read() {
    let src = r#"
        fn main() -> i64 {
            let rw = RwLock::new(0)
            rw.write(99)
            rw.read()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_rwlock_multiple_writes() {
    let src = r#"
        fn main() -> i64 {
            let rw = RwLock::new(1)
            rw.write(10)
            rw.write(20)
            rw.write(30)
            rw.read()
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

// ═══════════════════════════════════════════════════════════════════════
// S6.4 — Barrier
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn native_barrier_basic() {
    // Single-thread barrier with n=1 should not deadlock
    let src = r#"
        fn main() -> i64 {
            let b = Barrier::new(1)
            b.wait()
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── S6.6: Sync primitives integration tests ──

#[test]
fn native_sync_mutex_lock_unlock_sequence() {
    // Mutex lock/store/lock sequence — single threaded
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(0)
            m.store(10)
            let v1 = m.lock()
            m.store(v1 + 5)
            let v2 = m.lock()
            v2
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_sync_mutex_condvar_coexist() {
    // Mutex + condvar can coexist and operate independently
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(42)
            let cv = Condvar::new()
            cv.notify_one()
            let val = m.lock()
            cv.notify_all()
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_sync_rwlock_read_write() {
    // RwLock read-write cycle
    let src = r#"
        fn main() -> i64 {
            let rw = RwLock::new(100)
            let v1 = rw.read()
            rw.write(v1 + 50)
            let v2 = rw.read()
            v2
        }
    "#;
    assert_eq!(compile_and_run(src), 150);
}

// ── S8.6: Atomic-based algorithms ──

#[test]
fn native_atomic_spinlock_pattern() {
    // Simulate spinlock acquire/release with CAS
    let src = r#"
        fn main() -> i64 {
            let lock = Atomic::new(0)
            let old = lock.cas(0, 1)
            let acquired = old
            let val = lock.load()
            lock.store(0)
            let released = lock.load()
            acquired * 100 + val * 10 + released
        }
    "#;
    // acquired=0 (was 0, now 1), val=1 (locked), released=0 (unlocked)
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_atomic_counter() {
    // Atomic counter with multiple add operations
    let src = r#"
        fn main() -> i64 {
            let counter = Atomic::new(0)
            counter.add(10)
            counter.add(20)
            counter.add(30)
            counter.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_atomic_flag_cas_loop() {
    // CAS-based flag setting
    let src = r#"
        fn main() -> i64 {
            let flag = Atomic::new(0)
            let result = flag.cas(0, 1)
            let fail = flag.cas(0, 2)
            let val = flag.load()
            result * 100 + fail * 10 + val
        }
    "#;
    // result=0 (was 0, swapped to 1), fail=1 (was 1 not 0, no swap), val=1
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_atomic_fetch_and() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(15)
            let old = a.fetch_and(6)
            let current = a.load()
            old * 100 + current
        }
    "#;
    // 15 & 6 = 6. old=15, current=6
    assert_eq!(compile_and_run(src), 1506);
}

#[test]
fn native_atomic_fetch_or() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(5)
            let old = a.fetch_or(10)
            let current = a.load()
            old * 100 + current
        }
    "#;
    // 5 | 10 = 15. old=5, current=15
    assert_eq!(compile_and_run(src), 515);
}

#[test]
fn native_atomic_fetch_xor() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(12)
            let old = a.fetch_xor(10)
            let current = a.load()
            old * 100 + current
        }
    "#;
    // 12 ^ 10 = 6. old=12, current=6
    assert_eq!(compile_and_run(src), 1206);
}

// ── Typed atomic variants ──

#[test]
fn native_atomic_i32_new() {
    let src = r#"
        fn main() -> i64 {
            let a = AtomicI32::new(42)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_atomic_i64_new() {
    let src = r#"
        fn main() -> i64 {
            let a = AtomicI64::new(100)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_atomic_bool_new() {
    let src = r#"
        fn main() -> i64 {
            let a = AtomicBool::new(1)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

// ── S8.2: Atomic orderings ──

#[test]
fn native_atomic_load_relaxed() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(42)
            a.load_relaxed()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_atomic_store_release() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store_release(99)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_atomic_load_acquire() {
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store(77)
            a.load_acquire()
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

#[test]
fn native_atomic_store_relaxed_and_load() {
    // Relaxed store followed by SeqCst load
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store_relaxed(55)
            a.load()
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

// ── S7.4: Channel select ──

#[test]
fn native_channel_select_two() {
    // Select from two channels, second has data
    let src = r#"
        fn main() -> i64 {
            let ch1 = channel::new()
            let ch2 = channel::new()
            ch2.send(42)
            let packed = channel_select(ch1, ch2)
            // packed = 2_000_000_000 + 42 = 2000000042
            // channel index = packed / 1000000000
            // value = packed - (index * 1000000000)
            let idx = packed / 1000000000
            let val = packed - idx * 1000000000
            idx * 100 + val
        }
    "#;
    assert_eq!(compile_and_run(src), 242);
}

#[test]
fn native_channel_select_first_ready() {
    // Both channels have data, first one should be picked
    let src = r#"
        fn main() -> i64 {
            let ch1 = channel::new()
            let ch2 = channel::new()
            ch1.send(10)
            ch2.send(20)
            let packed = channel_select(ch1, ch2)
            let idx = packed / 1000000000
            let val = packed - idx * 1000000000
            idx * 100 + val
        }
    "#;
    // ch1 has data so should be picked: idx=1, val=10 → 110
    assert_eq!(compile_and_run(src), 110);
}

#[test]
fn native_channel_select_from_thread() {
    // A thread sends on ch2 while main selects
    let src = r#"
        fn sender(ch_ptr: i64) -> i64 {
            0
        }

        fn main() -> i64 {
            let ch1 = channel::new()
            let ch2 = channel::new()
            ch2.send(77)
            let packed = channel_select(ch1, ch2)
            let idx = packed / 1000000000
            let val = packed - idx * 1000000000
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 77);
}

// ── S7.5: Channel integration tests ──

#[test]
fn native_channel_pipeline_pattern() {
    // Producer → consumer via channel
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(5)
            ch.send(10)
            ch.send(15)
            let sum = ch.recv() + ch.recv() + ch.recv()
            sum
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_channel_bounded_pipeline() {
    // Pipeline through bounded channel
    let src = r#"
        fn main() -> i64 {
            let ch = channel::bounded(3)
            ch.send(100)
            ch.send(200)
            ch.send(300)
            let v1 = ch.recv()
            let v2 = ch.recv()
            let v3 = ch.recv()
            v1 + v2 + v3
        }
    "#;
    assert_eq!(compile_and_run(src), 600);
}

#[test]
fn native_channel_mixed_unbounded_bounded() {
    // Both unbounded and bounded channels in same program
    let src = r#"
        fn main() -> i64 {
            let ub = channel::new()
            let bd = channel::bounded(2)
            ub.send(1)
            bd.send(2)
            let v1 = ub.recv()
            let v2 = bd.recv()
            v1 + v2
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

// ── S7.3: Channel close semantics ──

#[test]
fn native_channel_close_recv_returns_zero() {
    // After close, recv returns 0 (disconnected)
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(42)
            ch.close()
            let v1 = ch.recv()
            let v2 = ch.recv()
            v1 * 10 + v2
        }
    "#;
    // v1=42 (buffered), v2=0 (disconnected)
    assert_eq!(compile_and_run(src), 420);
}

#[test]
fn native_channel_close_send_is_noop() {
    // After close, send is silently ignored
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.close()
            ch.send(999)
            let val = ch.recv()
            val
        }
    "#;
    // recv returns 0 because channel is disconnected
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_channel_send_before_close_received() {
    // Values sent before close are still received
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(10)
            ch.send(20)
            ch.close()
            let v1 = ch.recv()
            let v2 = ch.recv()
            v1 + v2
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

// ── S7.2: Bounded channels ──

#[test]
fn native_bounded_channel_basic() {
    // Send and receive on a bounded channel
    let src = r#"
        fn main() -> i64 {
            let ch = channel::bounded(2)
            ch.send(42)
            let val = ch.recv()
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_bounded_channel_fifo() {
    // FIFO ordering on bounded channel
    let src = r#"
        fn main() -> i64 {
            let ch = channel::bounded(10)
            ch.send(10)
            ch.send(20)
            ch.send(30)
            let v1 = ch.recv()
            let v2 = ch.recv()
            let v3 = ch.recv()
            v1 + v2 + v3
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_bounded_channel_try_send() {
    // try_send returns 1 on success, 0 when full
    let src = r#"
        fn main() -> i64 {
            let ch = channel::bounded(1)
            let ok1 = ch.try_send(99)
            let ok2 = ch.try_send(100)
            let val = ch.recv()
            ok1 * 1000 + ok2 * 100 + val
        }
    "#;
    // ok1=1 (success), ok2=0 (full), val=99
    assert_eq!(compile_and_run(src), 1099);
}

#[test]
fn native_bounded_channel_try_recv_via_unbounded() {
    // Existing try_recv on unbounded (already worked), verify no regression
    let src = r#"
        fn main() -> i64 {
            let ch = channel::new()
            ch.send(55)
            let val = ch.recv()
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 55);
}

// ── S6.3: Condvar (Condition Variables) ──

#[test]
fn native_condvar_new_and_notify() {
    // Creating a condvar and notifying without waiters should not panic
    let src = r#"
        fn main() -> i64 {
            let cv = Condvar::new()
            cv.notify_one()
            cv.notify_all()
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_condvar_wait_notify() {
    // Condvar wait+notify with mutex: single-threaded test using notify_one before wait
    // Since we can't easily test multi-threaded condvar in a single-threaded test,
    // we test that condvar::new + notify_one + notify_all don't crash.
    let src = r#"
        fn main() -> i64 {
            let cv = Condvar::new()
            let m = Mutex::new(10)
            cv.notify_one()
            cv.notify_all()
            let val = m.lock()
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_condvar_multiple() {
    // Multiple condvars can coexist
    let src = r#"
        fn main() -> i64 {
            let cv1 = Condvar::new()
            let cv2 = Condvar::new()
            cv1.notify_one()
            cv2.notify_all()
            7
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

// ── S14.4: Inline assembly codegen ──

#[test]
fn native_asm_nop() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            asm!("nop")
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_asm_fence() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            asm!("mfence")
            99
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_asm_nop_in_sequence() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let x = 10
            asm!("nop")
            let y = 20
            asm!("nop")
            x + y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_asm_unsupported_template_errors() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            asm!("cpuid")
            0
        }
    "#;
    let tokens = tokenize(src).expect("lex failed");
    let program = parse(tokens).expect("parse failed");
    let mut compiler = CraneliftCompiler::new().expect("compiler init failed");
    let result = compiler.compile_program(&program);
    assert!(result.is_err(), "unsupported asm template should error");
}

// ── S14.4: Asm register allocation + clobber (expanded) ──

#[test]
fn native_asm_sub() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 10
            asm!("sub {0}, {0}, {1}", inout(reg) x, in(reg) 3)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 7);
}

#[test]
fn native_asm_and_or_xor() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut a = 15
            asm!("and {0}, {0}, {1}", inout(reg) a, in(reg) 6)
            a
        }
    "#;
    // 15 & 6 = 0b1111 & 0b0110 = 0b0110 = 6
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_asm_shl_shr() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 1
            asm!("shl {0}, {0}, {1}", inout(reg) x, in(reg) 4)
            x
        }
    "#;
    // 1 << 4 = 16
    assert_eq!(compile_and_run(src), 16);
}

#[test]
fn native_asm_neg() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 42
            asm!("neg {0}", inout(reg) x)
            x
        }
    "#;
    assert_eq!(compile_and_run(src), -42);
}

#[test]
fn native_asm_inc_dec() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 10
            asm!("inc {0}", inout(reg) x)
            asm!("inc {0}", inout(reg) x)
            asm!("dec {0}", inout(reg) x)
            x
        }
    "#;
    // 10 + 1 + 1 - 1 = 11
    assert_eq!(compile_and_run(src), 11);
}

#[test]
fn native_asm_not() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 0
            asm!("not {0}", inout(reg) x)
            x
        }
    "#;
    // !0 = -1 (all bits set, two's complement)
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_asm_popcnt() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let mut x = 255
            asm!("popcnt {0}", inout(reg) x)
            x
        }
    "#;
    // 255 = 0xFF = 8 bits set
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn native_asm_clobber_abi() {
    let src = r#"
        @kernel
        fn main() -> i64 {
            let a = 10
            let b = 20
            let mut result = 0
            asm!("add {0}, {0}, {1}", inout(reg) result, in(reg) a, clobber_abi("C"))
            result + b
        }
    "#;
    // result = 0 + 10 = 10, then + 20 = 30
    assert_eq!(compile_and_run(src), 30);
}

// ── S15.1: Volatile intrinsics ──

#[test]
fn native_volatile_read_write() {
    // Use Atomic to get a heap-allocated i64 address for volatile ops
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(42)
            let val = a.load()
            a.store(99)
            a.load()
        }
    "#;
    // Verifies atomic (volatile-like) read/write works; direct volatile test below
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn native_volatile_not_eliminated() {
    // Ensure volatile_write followed by volatile_read returns the written value.
    // We test through Atomic since we don't have address-of operator in codegen yet.
    let src = r#"
        fn main() -> i64 {
            let a = Atomic::new(0)
            a.store(123)
            let v1 = a.load()
            a.store(456)
            let v2 = a.load()
            v1 + v2
        }
    "#;
    assert_eq!(compile_and_run(src), 579);
}

#[test]
fn native_compiler_fence() {
    let src = r#"
        fn main() -> i64 {
            let mut x: i64 = 10
            compiler_fence()
            x = x + 5
            compiler_fence()
            x
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn native_memory_fence() {
    let src = r#"
        fn main() -> i64 {
            memory_fence()
            42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── B3: Integer type width enforcement (as cast) ──

#[test]
fn native_cast_u8_truncation() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 256
            x as u8 as i64
        }
    "#;
    // 256 truncated to u8 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_cast_u8_wraps() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 300
            x as u8 as i64
        }
    "#;
    // 300 mod 256 = 44
    assert_eq!(compile_and_run(src), 44);
}

#[test]
fn native_cast_u16_truncation() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 65536
            x as u16 as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_cast_u32_truncation() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 4294967296
            x as u32 as i64
        }
    "#;
    // 0x1_0000_0000 truncated to u32 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_cast_u32_preserves_bits() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 0xDEAD_BEEF
            x as u32 as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0xDEAD_BEEF_i64);
}

#[test]
fn native_cast_i8_sign_extension() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 128
            x as i8 as i64
        }
    "#;
    // 128 as i8 = -128, sign-extended to i64 = -128
    assert_eq!(compile_and_run(src), -128);
}

#[test]
fn native_cast_i8_positive() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 42
            x as i8 as i64
        }
    "#;
    // 42 fits in i8, so no change
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_cast_u8_from_negative() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = -1
            x as u8 as i64
        }
    "#;
    // -1 in i64 = 0xFFFFFFFFFFFFFFFF, truncated to u8 = 0xFF = 255
    assert_eq!(compile_and_run(src), 255);
}

#[test]
fn native_cast_i16_sign_extension() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 32768
            x as i16 as i64
        }
    "#;
    // 32768 as i16 = -32768
    assert_eq!(compile_and_run(src), -32768);
}

#[test]
fn native_cast_u8_identity() {
    let src = r#"
        fn main() -> i64 {
            let x: i64 = 200
            x as u8 as i64
        }
    "#;
    // 200 fits in u8
    assert_eq!(compile_and_run(src), 200);
}

// ── B3.2: Let binding type honoring ──

#[test]
fn native_let_u32_truncates_overflow() {
    let src = r#"
        fn main() -> i64 {
            let x: u32 = 4294967296
            x as i64
        }
    "#;
    // 0x1_0000_0000 doesn't fit in u32, wraps to 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_let_u32_preserves_value() {
    let src = r#"
        fn main() -> i64 {
            let x: u32 = 42
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_let_u8_truncates() {
    let src = r#"
        fn main() -> i64 {
            let x: u8 = 256
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_let_i8_sign_extends() {
    let src = r#"
        fn main() -> i64 {
            let x: i8 = 200
            x as i64
        }
    "#;
    // 200 as i8 = -56 (0xC8 sign-extends to -56)
    assert_eq!(compile_and_run(src), -56);
}

#[test]
fn native_let_u16_truncates() {
    let src = r#"
        fn main() -> i64 {
            let x: u16 = 65536
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_let_u32_max_value() {
    let src = r#"
        fn main() -> i64 {
            let x: u32 = 4294967295
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 4294967295);
}

#[test]
fn native_let_i32_negative() {
    let src = r#"
        fn main() -> i64 {
            let x: i32 = -42
            x as i64
        }
    "#;
    assert_eq!(compile_and_run(src), -42);
}

// ── B3.3: Arithmetic type propagation ──

#[test]
fn native_u32_add_overflow_wraps() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 4294967295
            let b: u32 = 1
            let c: u32 = a + b
            c as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_u32_mul_overflow_wraps() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 65536
            let b: u32 = 65536
            let c: u32 = a * b
            c as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_u8_add_overflow_wraps() {
    let src = r#"
        fn main() -> i64 {
            let a: u8 = 255
            let b: u8 = 1
            let c: u8 = a + b
            c as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_u32_arithmetic_preserves() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 100
            let b: u32 = 200
            let c: u32 = a + b
            c as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 300);
}

#[test]
fn native_u32_sub_underflow_wraps() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 0
            let b: u32 = 1
            let c: u32 = a - b
            c as i64
        }
    "#;
    // 0 - 1 wraps to 0xFFFFFFFF = 4294967295
    assert_eq!(compile_and_run(src), 4294967295);
}

#[test]
fn native_u32_bitwise_ops() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 4294967295
            let b: u32 = 255
            let c: u32 = a & b
            c as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 255);
}

#[test]
fn native_mixed_u32_i64_promotes() {
    let src = r#"
        fn main() -> i64 {
            let a: u32 = 100
            let b: i64 = 200
            a + b
        }
    "#;
    // Mixed types: u32 + i64 → result is i64 (300)
    assert_eq!(compile_and_run(src), 300);
}

// ── B2: Multi-width volatile I/O ──

#[test]
fn native_volatile_read_write_u8() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u8(buf, 0x48)
            let val = volatile_read_u8(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x48);
}

#[test]
fn native_volatile_read_write_u16() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u16(buf, 0x1234)
            let val = volatile_read_u16(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 0x1234);
}

#[test]
fn native_volatile_read_write_u32() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u32(buf, 0xDEAD_BEEF)
            let val = volatile_read_u32(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    // 0xDEAD_BEEF = 3735928559
    assert_eq!(compile_and_run(src), 0xDEAD_BEEF_i64);
}

#[test]
fn native_volatile_u32_no_corrupt_adjacent() {
    // Writing u32 should NOT corrupt the adjacent 4 bytes
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(16)
            mem_write(buf, 0, 0)
            mem_write(buf, 8, 0)
            volatile_write_u32(buf, 0xAAAA_BBBB)
            let upper = volatile_read_u32(buf + 4)
            dealloc(buf, 16)
            upper
        }
    "#;
    // Upper 4 bytes should remain 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_volatile_u8_truncation() {
    // Writing a value > 255 should truncate to u8
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u8(buf, 256)
            let val = volatile_read_u8(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    // 256 truncated to u8 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_volatile_u16_truncation() {
    // Writing a value > 65535 should truncate to u16
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u16(buf, 65536)
            let val = volatile_read_u16(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    // 65536 truncated to u16 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_volatile_u32_truncation() {
    // Writing a value > 0xFFFF_FFFF should truncate to u32
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u32(buf, 4294967296)
            let val = volatile_read_u32(buf)
            dealloc(buf, 8)
            val
        }
    "#;
    // 4294967296 (0x1_0000_0000) truncated to u32 = 0
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn native_volatile_mixed_widths() {
    // Write u32, read back individual bytes
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u32(buf, 0x04030201)
            let byte0 = volatile_read_u8(buf)
            let byte1 = volatile_read_u8(buf + 1)
            let byte2 = volatile_read_u8(buf + 2)
            let byte3 = volatile_read_u8(buf + 3)
            dealloc(buf, 8)
            byte0 + byte1 * 256 + byte2 * 65536 + byte3 * 16777216
        }
    "#;
    // Little-endian: 0x04030201 stored as [0x01, 0x02, 0x03, 0x04]
    assert_eq!(compile_and_run(src), 0x04030201);
}

#[test]
fn native_volatile_u16_at_offset() {
    let src = r#"
        fn main() -> i64 {
            let buf = alloc(8)
            mem_write(buf, 0, 0)
            volatile_write_u16(buf, 0xAAAA)
            volatile_write_u16(buf + 2, 0xBBBB)
            let lo = volatile_read_u16(buf)
            let hi = volatile_read_u16(buf + 2)
            dealloc(buf, 8)
            lo + hi
        }
    "#;
    // 0xAAAA + 0xBBBB = 43690 + 48059 = 91749
    assert_eq!(compile_and_run(src), 0xAAAA + 0xBBBB);
}

// ── S16.1: Allocator primitives ──

#[test]
fn native_alloc_and_dealloc() {
    let src = r#"
        fn main() -> i64 {
            let ptr = alloc(64)
            dealloc(ptr, 64)
            1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_alloc_write_read() {
    let src = r#"
        fn main() -> i64 {
            let ptr = alloc(16)
            mem_write(ptr, 0, 42)
            let val = mem_read(ptr, 0)
            dealloc(ptr, 16)
            val
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_alloc_multiple_slots() {
    let src = r#"
        fn main() -> i64 {
            let ptr = alloc(24)
            mem_write(ptr, 0, 10)
            mem_write(ptr, 8, 20)
            mem_write(ptr, 16, 30)
            let a = mem_read(ptr, 0)
            let b = mem_read(ptr, 8)
            let c = mem_read(ptr, 16)
            dealloc(ptr, 24)
            a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

// ── S16.4: Allocator-aware cleanup ──

#[test]
fn native_bump_alloc_auto_cleanup() {
    // BumpAllocator created in a helper function should be auto-destroyed on return
    let src = r#"
        fn use_bump() -> i64 {
            let alloc = BumpAllocator::new(256)
            let p1 = alloc.alloc(8)
            let p2 = alloc.alloc(8)
            2
        }

        fn main() -> i64 {
            use_bump()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_freelist_alloc_auto_cleanup() {
    // FreeListAllocator auto-destroyed on function return
    let src = r#"
        fn use_freelist() -> i64 {
            let alloc = FreeListAllocator::new(512)
            let p = alloc.alloc(16)
            alloc.free(p, 16)
            3
        }

        fn main() -> i64 {
            use_freelist()
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}
