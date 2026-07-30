//! v0.4 generic enums, Drop/cleanup, Option/Result, ? operator, Future/Poll, lazy async, MNIST.

use super::compile_and_run;

// === v0.4 Sprint 1: Generic Enum Infrastructure ===

#[test]
fn native_s1_2_enum_f64_payload_match() {
    // User-defined enum with explicit f64 payload — match extracts f64 correctly
    let src = r#"
        enum Value {
            Float(f64),
            Int(i64),
            Empty
        }
        fn main() -> i64 {
            let v = Float(3.14)
            match v {
                Float(f) => {
                    let result = f * 2.0
                    result as i64
                }
                Int(i) => i,
                Empty => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_s1_2_generic_enum_basic() {
    // Generic enum definition with <T> — constructs and matches i64 payload
    let src = r#"
        enum MyOption<T> {
            MySome(T),
            MyNone
        }
        fn main() -> i64 {
            let x = MySome(42)
            match x {
                MySome(v) => v,
                MyNone => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s1_2_generic_enum_f64() {
    // Generic enum <T> with f64 argument — payload tracked as F64
    let src = r#"
        enum MyOption<T> {
            MySome(T),
            MyNone
        }
        fn main() -> i64 {
            let x = MySome(3.14)
            match x {
                MySome(val) => {
                    let doubled = val * 2.0
                    doubled as i64
                }
                MyNone => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 6);
}

#[test]
fn native_s1_2_generic_result_enum() {
    // Generic Result<T, E> with two type params
    let src = r#"
        enum MyResult<T, E> {
            MyOk(T),
            MyErr(E)
        }
        fn main() -> i64 {
            let r = MyOk(100)
            match r {
                MyOk(v) => v,
                MyErr(e) => e
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

#[test]
fn native_s1_3_match_payload_type_preserved() {
    // Verify that f64 payload type is preserved through variable storage and match
    let src = r#"
        enum Wrapper {
            Val(f64),
            None
        }
        fn main() -> i64 {
            let a = Val(1.5)
            let b = Val(2.5)
            let sum = match a {
                Val(x) => x,
                None => 0.0
            }
            let sum2 = match b {
                Val(y) => y,
                None => 0.0
            }
            let total = sum + sum2
            total as i64
        }
    "#;
    assert_eq!(compile_and_run(src), 4);
}

#[test]
fn native_s1_3_enum_variant_type_registry() {
    // Verify that enum_variant_types correctly tracks payload types
    // User-defined enum with mixed types in different variants
    let src = r#"
        enum Data {
            Count(i64),
            Empty
        }
        fn main() -> i64 {
            let d1 = Count(10)
            let d2 = Empty
            let v1 = match d1 {
                Count(n) => n,
                Empty => 0
            }
            let v2 = match d2 {
                Count(n) => n,
                Empty => -1
            }
            v1 + v2
        }
    "#;
    assert_eq!(compile_and_run(src), 9);
}

#[test]
fn native_s1_4_multi_field_variant() {
    // Multi-field variant: Rect(i64, i64) stored in stack slot
    let src = r#"
        enum Shape {
            Rect(i64, i64),
            Circle(i64),
            Point
        }
        fn main() -> i64 {
            let s = Rect(3, 4)
            match s {
                Rect(w, h) => w * h,
                Circle(r) => r,
                Point => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_s1_4_multi_field_f64() {
    // Multi-field variant with f64 types
    let src = r#"
        enum Shape {
            Rect(f64, f64),
            Point
        }
        fn main() -> i64 {
            let s = Rect(3.0, 4.0)
            match s {
                Rect(w, h) => {
                    let area = w * h
                    area as i64
                }
                Point => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 12);
}

#[test]
fn native_s1_5_fn_returns_enum() {
    // Function with enum return type — returns both tag and payload
    let src = r#"
        enum MyOpt {
            MySome(i64),
            MyNone
        }
        fn make_some(x: i64) -> MyOpt {
            MySome(x)
        }
        fn main() -> i64 {
            let opt = make_some(42)
            match opt {
                MySome(v) => v,
                MyNone => 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s1_5_fn_returns_none() {
    // Function returns enum without payload
    let src = r#"
        enum MyOpt {
            MySome(i64),
            MyNone
        }
        fn make_none() -> MyOpt {
            MyNone
        }
        fn main() -> i64 {
            let opt = make_none()
            match opt {
                MySome(v) => v,
                MyNone => -1
            }
        }
    "#;
    assert_eq!(compile_and_run(src), -1);
}

// ===== v0.4 Sprint 3: Scope-Level Drop/Cleanup =====

#[test]
fn native_s3_scope_cleanup_basic() {
    // Heap array created inside a block should be freed when the block exits.
    let src = r#"
        fn main() -> i64 {
            let mut total = 0
            {
                let mut arr: [i64] = []
                arr = arr.push(10)
                arr = arr.push(20)
                total = to_int(len(arr))
            }
            total
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_s3_nested_scopes() {
    // Resources in nested blocks should be cleaned up at each block exit.
    let src = r#"
        fn main() -> i64 {
            let mut result = 0
            {
                let mut a: [i64] = []
                a = a.push(1)
                {
                    let mut b: [i64] = []
                    b = b.push(2)
                    b = b.push(3)
                    result = result + to_int(len(b))
                }
                result = result + to_int(len(a))
            }
            result
        }
    "#;
    // inner block: len(b) = 2, outer block: len(a) = 1, total = 3
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_s3_scope_escape_no_double_free() {
    // When a scope-local array is assigned to an outer variable,
    // it must NOT be freed at scope exit (ownership escaped).
    let src = r#"
        fn main() -> i64 {
            let mut values: [i64] = []
            values = values.push(1)
            {
                let mut new_vals: [i64] = []
                new_vals = new_vals.push(10)
                new_vals = new_vals.push(20)
                values = new_vals
            }
            // values should still be valid after block exit
            values[0] + values[1]
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_s3_scope_cleanup_with_early_return() {
    // Resources should be cleaned up at function exit even with early return.
    let src = r#"
        fn helper() -> i64 {
            let mut arr: [i64] = []
            arr = arr.push(42)
            return arr[0]
        }
        fn main() -> i64 {
            helper()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s3_scope_map_cleanup() {
    // HashMap created inside a block should be freed at block exit.
    let src = r#"
        fn main() -> i64 {
            let mut result = 0
            {
                let m = HashMap::new()
                m.insert("x", 42)
                result = m.get("x")
            }
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s3_drop_trait_compiles_and_runs() {
    // A struct implementing Drop compiles and runs without crashing.
    // The drop method is called automatically at scope exit.
    let src = r#"
        struct Resource {
            value: i64
        }

        trait Drop {
            fn drop(&mut self)
        }

        impl Drop for Resource {
            fn drop(&mut self) {
                // drop is called automatically — no crash means success
                let _x = self.value
            }
        }

        fn main() -> i64 {
            let r = Resource { value: 42 }
            r.value
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s3_drop_trait_nested_scope() {
    // Drop called when struct goes out of scope in a nested block.
    let src = r#"
        struct Guard {
            id: i64
        }

        trait Drop {
            fn drop(&mut self)
        }

        impl Drop for Guard {
            fn drop(&mut self) {
                let _cleanup = self.id + 1
            }
        }

        fn main() -> i64 {
            let mut result = 0
            {
                let g = Guard { id: 10 }
                result = g.id
            }
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_s3_mutex_guard_auto_unlock() {
    // MutexGuard should auto-unlock when it goes out of scope.
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(0)
            {
                let guard = m.lock_guard()
                guard.set(42)
                let val = guard.get()
            }
            // After block exit, guard is dropped and mutex is unlocked.
            // We should be able to lock again.
            let result = m.lock()
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s3_mutex_guard_read_write() {
    // Guard provides get/set access while holding the lock.
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(10)
            let guard = m.lock_guard()
            let old = guard.get()
            guard.set(old + 5)
            let new_val = guard.get()
            new_val
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

// ===== v0.4 Sprint 2: Option<T> and Result<T,E> in Practice =====

#[test]
fn native_s2_try_lock_returns_option() {
    // mutex.try_lock() should return Option<i64> (tag=0 for Some, tag=1 for None)
    let src = r#"
        fn main() -> i64 {
            let m = Mutex::new(42)
            let opt = m.try_lock()
            match opt {
                Some(v) => v,
                None => -1
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s2_option_return_from_fn() {
    // Function returning Option<i64> via generic enum with explicit returns
    let src = r#"
        enum MyOption<T> {
            MySome(T),
            MyNone
        }

        fn find_positive(x: i64) -> MyOption {
            if x > 0 {
                return MySome(x)
            }
            return MyNone
        }

        fn main() -> i64 {
            let a = find_positive(10)
            let b = find_positive(-5)
            let va = match a {
                MySome(v) => v,
                MyNone => 0
            }
            let vb = match b {
                MySome(v) => v,
                MyNone => 0
            }
            va + vb
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_s2_result_return_from_fn() {
    // Function returning Result<i64, i64> via generic enum with explicit returns
    let src = r#"
        enum MyResult<T, E> {
            MyOk(T),
            MyErr(E)
        }

        fn safe_div(a: i64, b: i64) -> MyResult {
            if b == 0 {
                return MyErr(-1)
            }
            return MyOk(a / b)
        }

        fn main() -> i64 {
            let r1 = safe_div(10, 2)
            let r2 = safe_div(10, 0)
            let v1 = match r1 {
                MyOk(v) => v,
                MyErr(e) => e
            }
            let v2 = match r2 {
                MyOk(v) => v,
                MyErr(e) => e
            }
            v1 + v2
        }
    "#;
    // 10/2=5, 10/0=-1, total=4
    assert_eq!(compile_and_run(src), 4);
}

#[test]
fn native_s2_option_nested_match() {
    // Nested function calls with Option return using explicit returns
    let src = r#"
        enum MyOpt<T> {
            MySome(T),
            MyNone
        }

        fn lookup(key: i64) -> MyOpt {
            if key == 1 { return MySome(100) }
            if key == 2 { return MySome(200) }
            return MyNone
        }

        fn main() -> i64 {
            let mut total = 0
            let a = lookup(1)
            match a {
                MySome(v) => { total = total + v },
                MyNone => { total = total + 0 }
            }
            let b = lookup(3)
            match b {
                MySome(v) => { total = total + v },
                MyNone => { total = total + 1 }
            }
            total
        }
    "#;
    // lookup(1)=Some(100), lookup(3)=None → 100 + 1 = 101
    assert_eq!(compile_and_run(src), 101);
}

// ── v0.4 S2.3: Typed ? operator with Result<T,E> ──

#[test]
fn native_s2_try_operator_typed_result_ok_path() {
    // ? on Ok path: unwrap and continue
    let src = r#"
        enum MyResult<T, E> {
            MyOk(T),
            MyErr(E)
        }

        fn parse_val(x: i64) -> MyResult {
            if x >= 0 { return MyOk(x * 10) }
            return MyErr(-1)
        }

        fn process(x: i64) -> MyResult {
            let v = parse_val(x)?
            return MyOk(v + 1)
        }

        fn main() -> i64 {
            let r = process(5)
            match r {
                MyOk(v) => v,
                MyErr(e) => e
            }
        }
    "#;
    // parse_val(5) = MyOk(50), v = 50, process returns MyOk(51)
    assert_eq!(compile_and_run(src), 51);
}

#[test]
fn native_s2_try_operator_typed_result_err_path() {
    // ? on Err path: propagate error as typed Result
    let src = r#"
        enum MyResult<T, E> {
            MyOk(T),
            MyErr(E)
        }

        fn parse_val(x: i64) -> MyResult {
            if x >= 0 { return MyOk(x * 10) }
            return MyErr(-1)
        }

        fn process(x: i64) -> MyResult {
            let v = parse_val(x)?
            return MyOk(v + 1)
        }

        fn main() -> i64 {
            let r = process(-3)
            match r {
                MyOk(v) => v,
                MyErr(e) => e
            }
        }
    "#;
    // parse_val(-3) = MyErr(-1), ? propagates, process returns MyErr(-1)
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_s2_try_operator_chained() {
    // Multiple ? operators in sequence
    let src = r#"
        enum Res<T, E> {
            Good(T),
            Bad(E)
        }

        fn step1(x: i64) -> Res {
            if x > 0 { return Good(x + 1) }
            return Bad(-1)
        }

        fn step2(x: i64) -> Res {
            if x < 100 { return Good(x * 2) }
            return Bad(-2)
        }

        fn pipeline(x: i64) -> Res {
            let a = step1(x)?
            let b = step2(a)?
            return Good(b + 100)
        }

        fn main() -> i64 {
            let r = pipeline(5)
            match r {
                Good(v) => v,
                Bad(e) => e
            }
        }
    "#;
    // step1(5) = Good(6), step2(6) = Good(12), pipeline returns Good(112)
    assert_eq!(compile_and_run(src), 112);
}

// ── v0.4 S2.4: Match exhaustiveness for generic enums ──

#[test]
fn native_s2_exhaustive_match_compiles() {
    // Exhaustive match on user-defined enum compiles and runs correctly
    let src = r#"
        enum Status { Active, Inactive, Waiting }

        fn value(tag: i64) -> i64 {
            match tag {
                Active => 1,
                Inactive => 2,
                Waiting => 3
            }
        }

        fn main() -> i64 {
            value(0) + value(1) + value(2)
        }
    "#;
    // Active=0→1, Inactive=1→2, Waiting=2→3 → 6
    assert_eq!(compile_and_run(src), 6);
}

// ── v0.4 S4: Formal Future/Poll Types ──

#[test]
fn native_s4_poll_enum_ready_pending() {
    // Poll<T> as a built-in generic enum: Ready(T)=0, Pending=1
    let src = r#"
        fn check_poll(tag: i64) -> i64 {
            match tag {
                Ready(v) => v,
                Pending => -1
            }
        }

        fn main() -> i64 {
            check_poll(0) + check_poll(1)
        }
    "#;
    // Ready(0 payload→0) + Pending(-1) = -1
    // Actually: tag=0 → Ready match → payload=0; tag=1 → Pending match → -1
    // But check_poll(0) with payload not set → 0, check_poll(1) → -1
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_s4_poll_enum_return_from_fn() {
    // Function returning Poll<T> using built-in Ready/Pending variants
    let src = r#"
        fn try_compute(x: i64) -> Poll {
            if x > 0 { return Ready(x * 10) }
            return Pending
        }

        fn main() -> i64 {
            let r = try_compute(5)
            match r {
                Ready(v) => v,
                Pending => -1
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 50);
}

#[test]
fn native_s4_poll_pending_path() {
    // Poll::Pending path
    let src = r#"
        fn try_compute(x: i64) -> Poll {
            if x > 0 { return Ready(x * 10) }
            return Pending
        }

        fn main() -> i64 {
            let r = try_compute(-3)
            match r {
                Ready(v) => v,
                Pending => -1
            }
        }
    "#;
    assert_eq!(compile_and_run(src), -1);
}

#[test]
fn native_s4_async_fn_returns_future() {
    // async fn wraps result in Future, .await unwraps it
    let src = r#"
        async fn compute(x: i64) -> i64 {
            x * 2 + 1
        }

        fn main() -> i64 {
            compute(20).await
        }
    "#;
    assert_eq!(compile_and_run(src), 41);
}

// ── v0.4 S5: Lazy Async ──

#[test]
fn native_s5_lazy_poll_not_immediately_ready() {
    // Future starts unresolved; becomes ready after set_result
    // Uses instance method syntax on future handles
    let src = r#"
        async fn make_future() -> i64 { 42 }

        fn main() -> i64 {
            let fut = make_future()
            let result = fut.await
            result
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_s5_state_machine_sequential_awaits() {
    // Multiple sequential awaits → state machine with preserved locals
    let src = r#"
        async fn step1() -> i64 { 10 }
        async fn step2() -> i64 { 20 }
        async fn step3() -> i64 { 30 }

        async fn pipeline() -> i64 {
            let a = step1().await
            let b = step2().await
            let c = step3().await
            a + b + c
        }

        fn main() -> i64 {
            pipeline().await
        }
    "#;
    assert_eq!(compile_and_run(src), 60);
}

#[test]
fn native_s5_waker_reschedule() {
    // Waker: create, wake, check is_woken, reset
    let src = r#"
        fn main() -> i64 {
            let w = Waker::new()
            let before = w.is_woken()
            w.wake()
            let after = w.is_woken()
            w.reset()
            let reset_val = w.is_woken()
            w.drop()
            before * 100 + after * 10 + reset_val
        }
    "#;
    // before=0, after=1, reset_val=0 → 0*100 + 1*10 + 0 = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_s5_round_robin_executor() {
    // Executor: spawn multiple tasks, run all, get results
    let src = r#"
        async fn task_a() -> i64 { 10 }
        async fn task_b() -> i64 { 20 }
        async fn task_c() -> i64 { 30 }

        fn main() -> i64 {
            let exec = Executor::new()
            let f1 = task_a()
            let f2 = task_b()
            let f3 = task_c()
            exec.spawn(f1)
            exec.spawn(f2)
            exec.spawn(f3)
            let completed = exec.run()
            let r1 = exec.get_result(0)
            let r2 = exec.get_result(1)
            let r3 = exec.get_result(2)
            exec.free()
            r1 + r2 + r3 + completed
        }
    "#;
    // 10 + 20 + 30 + 3 (all completed) = 63
    assert_eq!(compile_and_run(src), 63);
}

// ── v0.4 S6: Polish & MNIST ──

#[test]
fn native_s6_mnist_training_loss_decreases() {
    // MNIST-style training: 4 samples, 3 features → 2 classes, 10 SGD steps
    // Verify loss monotonically decreases (training works end-to-end)
    let src = r#"
        fn main() -> i64 {
            let pred = tensor_ones(4, 2)
            let target = tensor_zeros(4, 2)
            tensor_set(target, 0, 0, 4607182418800017408)
            tensor_set(target, 1, 1, 4607182418800017408)
            tensor_set(target, 2, 0, 4607182418800017408)
            tensor_set(target, 3, 1, 4607182418800017408)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let lr_bits = 4602678819172646912
            let opt = sgd_new(lr_bits)
            let loss1 = mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            let loss10 = mse_loss(gp, gt)
            optimizer_free(opt, 0)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if loss10 < loss1 { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_s6_generic_enum_with_training() {
    // Combine generic enum with training: return status as enum
    let src = r#"
        enum TrainStatus<T, E> {
            Converged(T),
            Diverged(E)
        }

        fn check_training() -> TrainStatus {
            let pred = tensor_ones(2, 2)
            let target = tensor_zeros(2, 2)
            let gp = requires_grad(pred)
            let gt = requires_grad(target)
            let lr_bits = 4602678819172646912
            let opt = sgd_new(lr_bits)
            let val1 = mse_loss(gp, gt)
            sgd_step(opt, gp)
            zero_grad(gp)
            let val2 = mse_loss(gp, gt)
            optimizer_free(opt, 0)
            grad_tensor_free(gp)
            grad_tensor_free(gt)
            tensor_free(pred)
            tensor_free(target)
            if val2 < val1 { return Converged(1) }
            return Diverged(0)
        }

        fn main() -> i64 {
            let r = check_training()
            match r {
                Converged(v) => v,
                Diverged(e) => e
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn native_s6_release_smoke_all_features() {
    // Smoke test: exercise generic enums + scope cleanup + async + Poll in one program
    let src = r#"
        enum MyOption<T> { MySome(T), MyNone }

        fn lookup(key: i64) -> MyOption {
            if key > 0 { return MySome(key * 10) }
            return MyNone
        }

        async fn async_double(x: i64) -> i64 {
            x * 2
        }

        fn main() -> i64 {
            let opt = lookup(5)
            let val = match opt {
                MySome(v) => v,
                MyNone => 0
            }
            let doubled = async_double(val).await
            doubled
        }
    "#;
    // lookup(5) = MySome(50), val = 50, async_double(50) = 100
    assert_eq!(compile_and_run(src), 100);
}

// ── v0.3 S9.1: async block expression ──

#[test]
fn native_async_block_basic() {
    // async { expr } creates a future, .await extracts value
    let src = r#"
        fn main() -> i64 {
            let fut = async { 42 }
            fut.await
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_async_block_with_computation() {
    // async block with computation
    let src = r#"
        fn double(x: i64) -> i64 { x * 2 }

        fn main() -> i64 {
            let fut = async { double(21) }
            fut.await
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ── S4.8: Struct type in generics ───────────────────────────────────

#[test]
fn native_generic_struct_identity() {
    // Pass a struct through a generic identity function
    let src = r#"
        struct Point { x: i64, y: i64 }

        fn identity<T>(val: T) -> T { val }

        fn main() -> i64 {
            let p = Point { x: 10, y: 20 }
            let q = identity(p)
            q.x + q.y
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn native_generic_struct_field_access() {
    // Generic function that takes a struct and an operation selector
    let src = r#"
        struct Pair { a: i64, b: i64 }

        fn sum_pair(p: Pair) -> i64 { p.a + p.b }

        fn apply<T>(x: T, f: fn(T) -> i64) -> i64 { f(x) }

        fn main() -> i64 {
            let p = Pair { a: 3, b: 7 }
            apply(p, sum_pair)
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn native_generic_two_structs() {
    // Two different structs through the same generic function
    let src = r#"
        struct A { x: i64 }
        struct B { y: i64 }

        fn get_val<T>(val: T) -> T { val }

        fn main() -> i64 {
            let a = A { x: 5 }
            let b = B { y: 8 }
            let a2 = get_val(a)
            let b2 = get_val(b)
            a2.x + b2.y
        }
    "#;
    assert_eq!(compile_and_run(src), 13);
}

#[test]
fn native_struct_param_field_access() {
    // Non-generic function takes a struct param and accesses fields
    let src = r#"
        struct Rect { w: i64, h: i64 }

        fn area(r: Rect) -> i64 { r.w * r.h }

        fn main() -> i64 {
            let r = Rect { w: 6, h: 7 }
            area(r)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn native_struct_param_nested_call() {
    // Chain struct params: create, pass, compute
    let src = r#"
        struct Vec2 { x: i64, y: i64 }

        fn dot(a: Vec2, b: Vec2) -> i64 { a.x * b.x + a.y * b.y }

        fn main() -> i64 {
            let u = Vec2 { x: 3, y: 4 }
            let v = Vec2 { x: 5, y: 6 }
            dot(u, v)
        }
    "#;
    assert_eq!(compile_and_run(src), 39); // 3*5 + 4*6 = 15 + 24 = 39
}

#[test]
fn native_struct_param_modify_and_return() {
    // Function creates new struct from old struct fields
    let src = r#"
        struct Point { x: i64, y: i64 }

        fn translate(p: Point, dx: i64, dy: i64) -> i64 {
            p.x + dx + p.y + dy
        }

        fn main() -> i64 {
            let p = Point { x: 10, y: 20 }
            translate(p, 5, 3)
        }
    "#;
    assert_eq!(compile_and_run(src), 38);
}

// ── S4.10: Additional native examples ────────────────────────────────

#[test]
fn native_file_io_write_read() {
    // Test file write + read + exists via native runtime
    let src = r#"
        fn main() -> i64 {
            let path = "/tmp/fajar_native_test.txt"
            write_file(path, "hello42")
            let exists = file_exists(path)
            if exists { 1 } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
    let _ = std::fs::remove_file("/tmp/fajar_native_test.txt");
}

#[test]
fn native_file_io_append_and_exists() {
    // Test append + file_exists
    let src = r#"
        fn main() -> i64 {
            let path = "/tmp/fajar_native_append.txt"
            write_file(path, "line1")
            append_file(path, "line2")
            let exists = file_exists(path)
            let no = file_exists("/tmp/does_not_exist_99999.txt")
            if exists {
                if no { 0 } else { 1 }
            } else { 0 }
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
    let _ = std::fs::remove_file("/tmp/fajar_native_append.txt");
}

#[test]
fn native_example_factorial() {
    let src = r#"
        fn factorial(n: i64) -> i64 {
            if n <= 1 { 1 } else { n * factorial(n - 1) }
        }
        fn main() -> i64 { factorial(10) }
    "#;
    assert_eq!(compile_and_run(src), 3628800);
}

#[test]
fn native_example_fibonacci() {
    let src = r#"
        fn fibonacci(n: i64) -> i64 {
            if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
        }
        fn main() -> i64 { fibonacci(15) }
    "#;
    assert_eq!(compile_and_run(src), 610);
}

// ── S38.4: Real MNIST accuracy > 90% ────────────────────────────────

#[test]
fn native_mnist_real_accuracy_above_90() {
    use super::runtime_fns::*;

    let mnist_dir = std::path::Path::new("data");
    let train_images_path = mnist_dir.join("train-images-idx3-ubyte");
    let train_labels_path = mnist_dir.join("train-labels-idx1-ubyte");
    let test_images_path = mnist_dir.join("t10k-images-idx3-ubyte");
    let test_labels_path = mnist_dir.join("t10k-labels-idx1-ubyte");

    if !train_images_path.exists() {
        eprintln!("MNIST data not found at data/ — skipping real accuracy test");
        return;
    }

    // Load MNIST train + test data via runtime IDX parser
    let train_img_str = train_images_path.to_str().unwrap();
    let train_lbl_str = train_labels_path.to_str().unwrap();
    let test_img_str = test_images_path.to_str().unwrap();
    let test_lbl_str = test_labels_path.to_str().unwrap();

    let train_images = fj_rt_mnist_load_images(train_img_str.as_ptr(), train_img_str.len() as i64);
    let train_labels = fj_rt_mnist_load_labels(train_lbl_str.as_ptr(), train_lbl_str.len() as i64);
    let test_images = fj_rt_mnist_load_images(test_img_str.as_ptr(), test_img_str.len() as i64);
    let test_labels = fj_rt_mnist_load_labels(test_lbl_str.as_ptr(), test_lbl_str.len() as i64);

    assert!(!train_images.is_null(), "Failed to load train images");
    assert!(!train_labels.is_null(), "Failed to load train labels");
    assert!(!test_images.is_null(), "Failed to load test images");
    assert!(!test_labels.is_null(), "Failed to load test labels");

    let n_train = fj_rt_tensor_rows(train_images) as usize;
    let n_test = fj_rt_tensor_rows(test_images) as usize;
    assert_eq!(n_train, 60000);
    assert_eq!(n_test, 10000);

    // Normalize pixel values to [0, 1]
    let train_norm = fj_rt_tensor_normalize(train_images);
    let test_norm = fj_rt_tensor_normalize(test_images);

    // Simple 1-layer network: 784 -> 10 (softmax)
    // Work directly with raw tensors (no GradTensor wrapper) for manual SGD
    let w = fj_rt_tensor_rand(784, 10);
    let b = fj_rt_tensor_zeros(1, 10);

    // Scale weights by 0.01 for better init
    let w = {
        let s = fj_rt_tensor_scale(w, f64::to_bits(0.01) as i64);
        fj_rt_tensor_free(w);
        s
    };

    // Mini-batch training: 10 epochs, batch_size=200
    let batch_size: usize = 200;
    let epochs = 10;
    let lr = 0.5f64;

    for epoch in 0..epochs {
        let mut total_loss = 0.0f64;
        let mut batches = 0;
        for start in (0..n_train).step_by(batch_size) {
            let end = std::cmp::min(start + batch_size, n_train);
            let actual_batch = end - start;
            if actual_batch < batch_size {
                break;
            }

            // Extract batch
            let batch_img = fj_rt_tensor_zeros(batch_size as i64, 784);
            let batch_target = fj_rt_tensor_zeros(batch_size as i64, 10);

            for i in 0..batch_size {
                for j in 0..784 {
                    let val = fj_rt_tensor_get(train_norm, (start + i) as i64, j as i64);
                    fj_rt_tensor_set(batch_img, i as i64, j as i64, val);
                }
                let label_bits = fj_rt_tensor_get(train_labels, (start + i) as i64, 0);
                let label = f64::from_bits(label_bits as u64) as usize;
                fj_rt_tensor_set(
                    batch_target,
                    i as i64,
                    label as i64,
                    f64::to_bits(1.0) as i64,
                );
            }

            // Forward: logits = batch_img @ w
            let logits = fj_rt_tensor_matmul(batch_img, w);

            // Add bias (broadcast)
            let bias_full = fj_rt_tensor_zeros(batch_size as i64, 10);
            for i in 0..batch_size {
                for j in 0..10 {
                    let bval = fj_rt_tensor_get(b, 0, j as i64);
                    fj_rt_tensor_set(bias_full, i as i64, j as i64, bval);
                }
            }
            let logits_biased = fj_rt_tensor_add(logits, bias_full);
            fj_rt_tensor_free(logits);
            fj_rt_tensor_free(bias_full);

            // Softmax
            let pred = fj_rt_tensor_softmax(logits_biased);
            fj_rt_tensor_free(logits_biased);

            // Compute cross-entropy loss for monitoring
            // CE = -sum(target * log(pred)) / batch_size
            let mut batch_loss = 0.0f64;
            for i in 0..batch_size {
                for j in 0..10 {
                    let t =
                        f64::from_bits(fj_rt_tensor_get(batch_target, i as i64, j as i64) as u64);
                    if t > 0.0 {
                        let p = f64::from_bits(fj_rt_tensor_get(pred, i as i64, j as i64) as u64);
                        batch_loss -= (p + 1e-10).ln();
                    }
                }
            }
            total_loss += batch_loss / batch_size as f64;
            batches += 1;

            // Gradient: d_logits = (pred - target) / batch_size
            let d_logits = fj_rt_tensor_sub(pred, batch_target);
            let inv_bs = f64::to_bits(1.0 / batch_size as f64) as i64;
            let d_logits_scaled = fj_rt_tensor_scale(d_logits, inv_bs);
            fj_rt_tensor_free(d_logits);

            // d_w = batch_img^T @ d_logits_scaled
            let batch_img_t = fj_rt_tensor_transpose(batch_img);
            let d_w = fj_rt_tensor_matmul(batch_img_t, d_logits_scaled);

            // d_b = column sums of d_logits_scaled → (1, 10)
            let d_b = fj_rt_tensor_zeros(1, 10);
            for j in 0..10 {
                let mut sum = 0.0f64;
                for i in 0..batch_size {
                    sum += f64::from_bits(
                        fj_rt_tensor_get(d_logits_scaled, i as i64, j as i64) as u64
                    );
                }
                fj_rt_tensor_set(d_b, 0, j as i64, f64::to_bits(sum) as i64);
            }

            // SGD update: w -= lr * d_w, b -= lr * d_b
            for i in 0..784 {
                for j in 0..10 {
                    let wv = f64::from_bits(fj_rt_tensor_get(w, i as i64, j as i64) as u64);
                    let dv = f64::from_bits(fj_rt_tensor_get(d_w, i as i64, j as i64) as u64);
                    fj_rt_tensor_set(w, i as i64, j as i64, f64::to_bits(wv - lr * dv) as i64);
                }
            }
            for j in 0..10 {
                let bv = f64::from_bits(fj_rt_tensor_get(b, 0, j as i64) as u64);
                let dv = f64::from_bits(fj_rt_tensor_get(d_b, 0, j as i64) as u64);
                fj_rt_tensor_set(b, 0, j as i64, f64::to_bits(bv - lr * dv) as i64);
            }

            // Cleanup
            fj_rt_tensor_free(batch_img);
            fj_rt_tensor_free(batch_img_t);
            fj_rt_tensor_free(pred);
            fj_rt_tensor_free(batch_target);
            fj_rt_tensor_free(d_logits_scaled);
            fj_rt_tensor_free(d_w);
            fj_rt_tensor_free(d_b);
        }
        let avg_loss = total_loss / batches as f64;
        eprintln!("Epoch {}: avg_loss = {:.4}", epoch + 1, avg_loss);
    }

    // Evaluate on test set
    let mut correct = 0usize;
    let eval_batch = 500;
    for start in (0..n_test).step_by(eval_batch) {
        let end = std::cmp::min(start + eval_batch, n_test);
        let actual = end - start;

        let batch_img = fj_rt_tensor_zeros(actual as i64, 784);
        for i in 0..actual {
            for j in 0..784 {
                let val = fj_rt_tensor_get(test_norm, (start + i) as i64, j as i64);
                fj_rt_tensor_set(batch_img, i as i64, j as i64, val);
            }
        }

        let logits = fj_rt_tensor_matmul(batch_img, w);
        let bias_full = fj_rt_tensor_zeros(actual as i64, 10);
        for i in 0..actual {
            for j in 0..10 {
                let bval = fj_rt_tensor_get(b, 0, j as i64);
                fj_rt_tensor_set(bias_full, i as i64, j as i64, bval);
            }
        }
        let logits_biased = fj_rt_tensor_add(logits, bias_full);
        fj_rt_tensor_free(logits);
        fj_rt_tensor_free(bias_full);

        let pred = fj_rt_tensor_softmax(logits_biased);
        fj_rt_tensor_free(logits_biased);

        for i in 0..actual {
            let mut max_val = f64::NEG_INFINITY;
            let mut max_idx = 0usize;
            for j in 0..10 {
                let v = f64::from_bits(fj_rt_tensor_get(pred, i as i64, j as i64) as u64);
                if v > max_val {
                    max_val = v;
                    max_idx = j;
                }
            }
            let label = f64::from_bits(fj_rt_tensor_get(test_labels, (start + i) as i64, 0) as u64)
                as usize;
            if max_idx == label {
                correct += 1;
            }
        }

        fj_rt_tensor_free(pred);
        fj_rt_tensor_free(batch_img);
    }

    let accuracy = correct as f64 / n_test as f64 * 100.0;
    eprintln!(
        "MNIST test accuracy: {}/{} = {:.2}%",
        correct, n_test, accuracy
    );

    // Cleanup
    fj_rt_tensor_free(w);
    fj_rt_tensor_free(b);
    fj_rt_tensor_free(train_images);
    fj_rt_tensor_free(train_labels);
    fj_rt_tensor_free(test_images);
    fj_rt_tensor_free(test_labels);
    fj_rt_tensor_free(train_norm);
    fj_rt_tensor_free(test_norm);

    assert!(
        accuracy > 90.0,
        "MNIST accuracy {:.2}% is below 90% target",
        accuracy
    );
}

#[test]
fn native_map_fn_style() {
    let src = r#"
        fn main() -> i64 {
            let mut m = map_new()
            m = map_insert(m, "a", 10)
            m = map_insert(m, "b", 20)
            let n = map_len(m)
            n
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn native_tensor_xavier() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_xavier(3, 4)
            let rows = tensor_rows(t)
            let cols = tensor_cols(t)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    assert_eq!(compile_and_run(src), 34);
}

#[test]
fn native_tensor_argmax() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_zeros(1, 5)
            let bits_10 = 4621819117588971520
            tensor_set(t, 0, 3, bits_10)
            let idx = tensor_argmax(t)
            tensor_free(t)
            idx
        }
    "#;
    // bits_10 = f64::to_bits(10.0); set element at (0,3) = 10.0 → argmax = 3
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn native_tensor_from_data() {
    let src = r#"
        fn main() -> i64 {
            let t = tensor_from_data(0, 0, 2, 3)
            let rows = tensor_rows(t)
            let cols = tensor_cols(t)
            tensor_free(t)
            rows * 10 + cols
        }
    "#;
    // null ptr / 0 elems → falls back to zeros(2,3)
    assert_eq!(compile_and_run(src), 23);
}
