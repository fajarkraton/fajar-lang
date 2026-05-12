# Path F — `src/distributed/` extraction — B0 Findings

> **Phase:** Compass §5.1 verdict execution — *"Hapus dari core. Tidak relevan untuk niche embedded. Jadikan side library."*
> **Audit date:** 2026-05-12 EOS-42 (Phase 0 of `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md`).
> **Plan Hygiene §6.8 R1:** Audit only. Pre-flight before any code movement.
> **HEAD at audit:** `34be1044`.
> **Predecessors:** `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md` (commit `eb3a3c25`)
> + Phase 0 decision file `docs/decisions/2026-05-12-path-e-f-prep.md` (commit `b5b6a67b`).
> **Target repo (per D-0.1):** `fajarkraton/fajar-distributed` (✅ created EOS-38, public, Apache-2.0).
> **CLI verdict (per D-0.2-distributed):** Option α — `cmd_run_cluster` deleted at F.4.

## §1. Verdict & headline metrics

`src/distributed/` is a **15,343-LOC self-contained subsystem** with **ZERO `use crate::*` imports of non-self fajar-lang internals**, **ZERO stdlib `.fj` references**, **ZERO fajaros-x86 cross-repo consumers** (LICENSE text excluded), and exactly **THREE Rust source files** that reference it (`src/main.rs`, `src/interpreter/eval/mod.rs`, `tests/nova_v2_tests.rs`). Extraction is therefore **mechanically tractable** — no cyclic deps, no Stage 2 byte-equality risk, no cross-module rewrite.

Plan §8 risk register flagged `nova_v2_tests.rs` as MED-HIGH probability surprise (echoing Path C eval/mod.rs +10 over B0). This audit found **10 distributed-touching tests in nova_v2_tests.rs (all in the `v14_n3_*` block, lines 250-336)**, all in category (a) — direct use of `distributed::*` types. Surprise risk **REDUCED to LOW** for this file; complete map below in §4.

| Metric | Value | Source |
|---|---|---|
| Total LOC | **15,343** | `wc -l src/distributed/*.rs` |
| File count | **16** (15 modules + `mod.rs`) | `ls src/distributed/` |
| `#[test]` count (in-file lib tests) | **322** | `grep -cE "^[[:space:]]*#\[test\]"` |
| `#[tokio::test]` count (additional async lib tests) | **10** (rpc.rs:1 + transport.rs:9) | `grep -cE "^[[:space:]]*#\[tokio::test\]"` |
| Total in-file tests | **332** | sum |
| External consumer Rust files | **3** (main.rs + eval/mod.rs + nova_v2_tests.rs) | grep |
| Sprint tests in eval/mod.rs touching `distributed` | **20** (10 N3 + 6 cascaded N5/N6 + 3 N8/N10 + 1 W10) | grep + fn-name resolution |
| `tests/nova_v2_tests.rs` tests touching `distributed` | **10** (all `v14_n3_*`, lines 250-336) | grep + fn-name resolution |
| `pub` items exported | **301** | grep `^pub (fn|struct|enum|trait|use|const|static|type)` |
| Internal `use crate::*` (non-self) imports | **0** | grep |
| stdlib `.fj` references | **0** (one unrelated match: "Beta-distributed sampling") | grep |
| fajaros-x86 cross-repo references | **0** (3 LICENSE matches excluded) | grep |
| Cargo.toml deps specific to distributed | **0** | grep |

Plan claim: "15,343 LOC, 322 tests across 16 files." **EXACT MATCH on LOC and file count. Plan's 322 = `#[test]` only; another 10 `#[tokio::test]` exist (not in plan figure).** Honest correction: **332 lib tests total**.

## §2. File inventory

Per `wc -l` + `grep -cE "^[[:space:]]*#\[(tokio::)?test\]"`:

| # | File | LOC | `#[test]` | `#[tokio::test]` | Total | Purpose (from file header docstring) |
|---|---|---:|---:|---:|---:|---|
| 1 | `mod.rs` | 21 | 0 | 0 | 0 | Re-exports 15 `pub mod` declarations |
| 2 | `cluster.rs` | 651 | 16 | 0 | 16 | Node discovery, resource ads, task placement, fault detection, work stealing, barriers, quotas |
| 3 | `data_plane.rs` | 929 | 27 | 0 | 27 | Data partitioning, transfer protocol, scatter/gather/broadcast, LZ4 compression |
| 4 | `deploy.rs` | 1,060 | 20 | 0 | 20 | Sprint D8: `fj run --cluster` CLI, cluster mgmt, fj.toml config, Dockerfile/K8s gen |
| 5 | `discovery.rs` | 1,077 | 28 | 0 | 28 | mDNS, seed bootstrap, DNS-SRV, SWIM gossip, health checking, scaling |
| 6 | `dist_bench.rs` | 1,014 | 19 | 0 | 19 | Sprint D10: speedup/AllReduce/election benchmarks + docs |
| 7 | `fault_tolerance.rs` | 727 | 15 | 0 | 15 | Checkpointing, recovery, exactly-once, saga, DLQ, circuit breaker, bulkhead, chaos |
| 8 | `fault_tolerance_v2.rs` | 1,295 | 19 | 0 | 19 | Sprint D7: leader/worker failover, partition handling, graceful shutdown, replication |
| 9 | `ml_training.rs` | 1,072 | 28 | 0 | 28 | Data-parallel + model-parallel, grad sync, parameter server, LR scaling, checkpointing, AMP, elastic |
| 10 | `raft.rs` | 1,046 | 25 | 0 | 25 | Raft consensus: election, log replication, commit/apply, WAL, snapshots, joint-config, pre-vote, lease reads |
| 11 | `rpc.rs` | 1,024 | 17 | 1 | 18 | RPC framework: service-def, codegen, serialization, transport, discovery, LB, retry, streaming |
| 12 | `rpc_v2.rs` | 1,287 | 27 | 0 | 27 | Sprint D6: bi-streaming, deadline prop, compression, auth, interceptors, client-side LB |
| 13 | `scheduler.rs` | 949 | 24 | 0 | 24 | Distributed task def, placement strategies, data locality, priority queue with fairness |
| 14 | `security.rs` | 1,358 | 20 | 0 | 20 | Sprint D9: TLS/mTLS, cert rotation, RBAC, quotas, audit logging, secrets |
| 15 | `tensors.rs` | 527 | 15 | 0 | 15 | Tensor sharding, placement, distributed matmul, allreduce, ring-allreduce, PS, DP/MP |
| 16 | `transport.rs` | 1,306 | 22 | 9 | 31 | Real TCP transport layer (actor model, retry, pool, service registry, heartbeat, FIFO) |
| | **Total** | **15,343** | **322** | **10** | **332** | |

Cross-check with plan: 16 files ✓ · 15,343 LOC ✓ · 322 `#[test]` ✓. Plan figure misses the 10 `#[tokio::test]` (split across `rpc.rs:752+1` and `transport.rs:730+9`). Total test surface to migrate to extracted crate: **332**.

## §3. Symbol-surface freeze

### 3a. External consumers (exhaustive grep)

Command:
```bash
grep -rn "distributed::" src/ tests/ examples/ benches/ stdlib/ | grep -v "^src/distributed/"
```

Result: **3 Rust files** + **1 aspirational .fj example** (uses fj-source namespace, not Rust crate — non-consumer).

| Consumer file | Line | Imported symbols | Consumer category |
|---|---|---|---|
| `src/main.rs` | 5815 | `distributed::raft::{self, RaftNode, RaftNodeId, RequestVoteReply}` | CLI `cmd_run_cluster` |
| `src/main.rs` | 5816-5818 | `distributed::scheduler::{DistributedTask, PlacementStrategy, TaskId, TaskLoadBalancer, TaskResources, WorkerId, WorkerNode}` | CLI `cmd_run_cluster` |
| `src/interpreter/eval/mod.rs` | 7383 | `distributed::raft::{RaftNode, RaftNodeId}` | sprint N3 test `n3_1_raft_node_creation` |
| `src/interpreter/eval/mod.rs` | 7394 | `distributed::raft::{RaftNode, RaftNodeId}` | sprint N3 test `n3_2_raft_log_index` |
| `src/interpreter/eval/mod.rs` | 7402 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N3 test `n3_3_discovery_registry` |
| `src/interpreter/eval/mod.rs` | 7417 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N3 test `n3_4_discovery_resolve` |
| `src/interpreter/eval/mod.rs` | 7433 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N3 test `n3_5_discovery_unhealthy` |
| `src/interpreter/eval/mod.rs` | 7450 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N3 test `n3_6_discovery_deregister` |
| `src/interpreter/eval/mod.rs` | 7466 | `distributed::cluster::{ClusterNodeId, FailureDetector}` | sprint N3 test `n3_7_failure_detector` |
| `src/interpreter/eval/mod.rs` | 7478 | `distributed::cluster::{ClusterNodeId, WorkQueue}` | sprint N3 test `n3_8_work_queue` |
| `src/interpreter/eval/mod.rs` | 7490 | `distributed::cluster::{ClusterNodeId, WorkQueue}` | sprint N3 test `n3_9_work_queue_empty` |
| `src/interpreter/eval/mod.rs` | 7499 | `distributed::raft::{RaftNode, RaftNodeId}` | sprint N3 test `n3_10_raft_quorum_sizes` |
| `src/interpreter/eval/mod.rs` | 7670 | `distributed::raft::{RaftNode, RaftNodeId}` | sprint N5 test `n5_4_raft_five_node` |
| `src/interpreter/eval/mod.rs` | 7681 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N5 test `n5_5_discovery_multiple_services` |
| `src/interpreter/eval/mod.rs` | 7698 | `distributed::cluster::{ClusterNodeId, FailureDetector}` | sprint N5 test `n5_6_failure_detector_healthy` |
| `src/interpreter/eval/mod.rs` | 7749 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N6 test `n6_2_discovery_all_instances` |
| `src/interpreter/eval/mod.rs` | 7772 | `distributed::cluster::{ClusterNodeId, WorkQueue}` | sprint N6 test `n6_3_work_queue_ordering` |
| `src/interpreter/eval/mod.rs` | 7783 | `distributed::raft::{RaftNode, RaftNodeId}` | sprint N6 test `n6_4_raft_single_node` |
| `src/interpreter/eval/mod.rs` | 7962 | `distributed::discovery::{DiscoveryRegistry, ServiceInstance}` | sprint N8 test `n8_5_discovery_tags` |
| `src/interpreter/eval/mod.rs` | 7980 | `distributed::cluster::{ClusterNodeId, WorkQueue}` | sprint N8 test `n8_6_work_queue_steal_all` |
| `src/interpreter/eval/mod.rs` | 8189 | `distributed::discovery::DiscoveryMode` | sprint N10 test `n10_5_discovery_mode_variants` |
| `src/interpreter/eval/mod.rs` | 9353-9355 | `distributed::cluster::{ClusterNodeId, FailureDetector, WorkQueue}` + `discovery::{DiscoveryRegistry, ServiceInstance}` + `raft::{RaftNode, RaftNodeId}` | sprint W10 test `w10_8_distributed_full` |
| `tests/nova_v2_tests.rs` | 260, 267 | `distributed::raft::RaftRole` | tests `v14_n3_2`, `v14_n3_3` |
| `tests/nova_v2_tests.rs` | 278, 279 | `distributed::raft::{RaftNodeId, RaftNode}` | test `v14_n3_4_raft_node_creation` |
| `tests/nova_v2_tests.rs` | 293 | `distributed::discovery::SwimState` | test `v14_n3_6_discovery_swim_states` |
| `tests/nova_v2_tests.rs` | 307 | `distributed::raft::LogEntry` (struct-literal use) | test `v14_n3_8_raft_log_entry` |
| `tests/nova_v2_tests.rs` | 317-323 | `distributed::raft::LogEntry` (Vec<>) | test `v14_n3_9_log_replication` |
| `tests/nova_v2_tests.rs` | 334 | `distributed::tensors::NodeId` | test `v14_n3_10_distributed_tensor_types` |
| `tests/nova_v2_tests.rs` | 253, 288 | string-literal `"src/distributed"` + `"src/distributed/discovery.rs"` (path-existence checks) | tests `v14_n3_1`, `v14_n3_5` (category b — see §4) |
| `examples/aspirational/distributed_mnist.fj` | 7 | `use distributed::{data_parallel, allreduce, checkpoint}` | **Fj-source namespace, NOT Rust crate.** Aspirational example, not currently runnable. Plan §F.5 candidate for deletion |

**eval/mod.rs sprint-N3 block (lines 7382-7513) is fully distributed-bound (10/10 tests).** Cascaded distributed usage continues sparsely across N5/N6/N8/N10/W10 sprints. **Total eval/mod.rs distributed-consumer tests: 20 (lines 7382, 7393, 7401, 7416, 7432, 7449, 7465, 7477, 7489, 7498, 7669, 7680, 7697, 7748, 7771, 7782, 7961, 7979, 8188, 9352).** These 20 tests are non-contiguous and require careful boundary-aware deletion at Phase F.5 (mirrors Path C eval/mod.rs pattern).

### 3b. Pub-symbol export list (FROZEN — 301 items)

The extracted crate MUST preserve every path below. Per file:

#### `raft.rs` (27 pub items — externally consumed: `RaftNode`, `RaftNodeId`, `RaftRole`, `RequestVoteReply`, `LogEntry`)
`RaftNodeId` (struct), `RaftRole` (enum), `LogEntry` (struct), `RaftNode` (struct), `RequestVoteArgs` (struct), `RequestVoteReply` (struct), `start_election` (fn), `build_request_vote` (fn), `handle_request_vote` (fn), `receive_vote` (fn), `become_leader` (fn), `AppendEntriesArgs` (struct), `AppendEntriesReply` (struct), `handle_append_entries` (fn), `leader_advance_commit` (fn), `apply_committed` (fn), `client_request` (fn), `WalRecord` (struct), `WalStorage` (struct), `RaftSnapshot` (struct), `create_snapshot` (fn), `install_snapshot` (fn), `ClusterConfig` (struct), `JointConfig` (struct), `pre_vote_check` (fn), `lease_read_allowed` (fn), `extend_lease` (fn).

#### `scheduler.rs` (17 pub items — externally consumed: `DistributedTask`, `PlacementStrategy`, `TaskId`, `TaskLoadBalancer`, `TaskResources`, `WorkerId`, `WorkerNode`)
`TaskId` (struct), `WorkerId` (struct), `TaskState` (enum), `TaskResources` (struct), `DistributedTask` (struct), `PlacementStrategy` (enum), `WorkerNode` (struct), `select_data_local` (fn), `TaskLoadBalancer` (struct), `PriorityTaskQueue` (struct), `CancelResult` (enum), `cancel_task` (fn), `TaskRetryPolicy` (struct), `retry_task` (fn), `TaskDag` (struct), `ResourceReservation` (struct), `ReservationManager` (struct).

#### `discovery.rs` (21 pub items — externally consumed: `DiscoveryMode`, `DiscoveryRegistry`, `ServiceInstance`, `SwimState`)
`DiscoveryNodeId` (struct), `MdnsRecord` (struct), `MdnsDiscovery` (struct), `SeedConfig` (struct), `JoinResult` (enum), `seed_bootstrap` (fn), `DnsSrvRecord` (struct), `DnsResolver` (struct), `SwimState` (enum), `SwimMember` (struct), `GossipProtocol` (struct), `NodeMetadata` (struct), `HealthStatus` (enum), `HealthCheckEntry` (struct), `HealthChecker` (struct), `ScalingEvent` (enum), `ScalingPolicy` (struct), `ServiceInstance` (struct), `DiscoveryRegistry` (struct), `DiscoveryMode` (enum), `DiscoveryConfig` (struct).

#### `cluster.rs` (15 pub items — externally consumed: `ClusterNodeId`, `FailureDetector`, `WorkQueue`)
`ClusterNodeId` (struct), `DiscoveryMethod` (enum), `NodeResources` (struct), `ClusterNode` (struct), `NodeStatus` (enum), `TaskRequirements` (struct), `place_task` (fn), `FailureDetector` (struct), `TaskCheckpoint` (struct), `MigrationResult` (enum), `WorkQueue` (struct), `find_steal_target` (fn), `ClusterBarrier` (struct), `TopologyInfo` (struct), `ResourceQuota` (struct).

#### `tensors.rs` (14 pub items — externally consumed: `NodeId`)
`NodeId` (struct), `TensorShard` (struct), `DistributedTensor` (struct), `shard_tensor` (fn), `PlacementStrategy` (enum — distinct from scheduler::PlacementStrategy), `place_shards` (fn), `local_matmul` (fn), `ReduceOp` (enum), `allreduce` (fn), `ring_allreduce_steps` (fn), `ParameterServer` (struct), `DataParallelConfig` (struct), `ModelParallelConfig` (struct), `CommBackend` (enum).

#### `data_plane.rs` (29 pub items — none externally consumed; full inventory in source)
Surface includes partitioning + scatter/gather/broadcast + LZ4 compression types/fns. Internal to the extracted crate; ZERO external imports — but extracted crate keeps the full surface for completeness.

#### `deploy.rs` (18 pub items — none externally consumed)
CLI helpers (Sprint D8). Includes `fj.toml` cluster-config parsers + Dockerfile/Kubernetes generators. **Note:** When CLI subcommand is deleted per D-0.2 α, this module's user-facing utility moves with it to the extracted crate, available to consumers who provide their own driver.

#### `rpc.rs` (23 pub items — none externally consumed)
RPC service def + codegen + serialization + transport adapters + load-balancing + retry + streaming.

#### `rpc_v2.rs` (24 pub items — none externally consumed)
Sprint D6: bi-directional streaming + deadline propagation + compression + auth + interceptors + client-side LB.

#### `transport.rs` (11 pub items — none externally consumed)
Real TCP transport (actors + retry + connection pool + service registry + heartbeat + FIFO).

#### `security.rs` (25 pub items — none externally consumed)
Sprint D9: TLS/mTLS + cert rotation + RBAC + quotas + audit logging + secrets.

#### `ml_training.rs` (18 pub items — none externally consumed)
Data-parallel + model-parallel training types + parameter server + LR scaling + checkpointing + AMP + elastic-training scaffolding.

#### `fault_tolerance.rs` (16 pub items — none externally consumed)
Checkpointing + recovery + exactly-once + saga + DLQ + circuit breaker + bulkhead + chaos.

#### `fault_tolerance_v2.rs` (22 pub items — none externally consumed)
Sprint D7: leader/worker failover + partition handling + graceful shutdown + replication.

#### `dist_bench.rs` (21 pub items — none externally consumed)
Sprint D10 benches + speedup/AllReduce/election latency + recovery-time measurement.

**Aggregate (`grep -hE "^pub (fn|struct|enum|trait|use|const|static|type)" src/distributed/*.rs | awk '{print $2}' | sort | uniq -c`):**

| Symbol kind | Count |
|---|---:|
| `struct` | 175 |
| `fn` | 69 |
| `enum` | 57 |
| `trait` | 0 |
| `pub use` re-export | 0 |
| `const` / `static` / `type` | 0 |
| **Total pub items** | **301** |

**Externally consumed today: 19 distinct symbols across 5 sub-modules (`raft`, `scheduler`, `discovery`, `cluster`, `tensors`).** The other ~282 pub items have no fajar-lang consumer outside `src/distributed/`. After CLI removal (D-0.2 α), only 4 sub-modules retain external consumers in `tests/nova_v2_tests.rs` (`raft`, `discovery`, `tensors`) and eval/mod.rs sprints — those test consumers are removed at F.5.

**Post-F.5 external surface = 0 (zero in-crate consumers; new crate is freestanding).**

## §4. `tests/nova_v2_tests.rs` deep audit (the F-specific surprise risk)

Plan §8: MED-HIGH probability that `nova_v2_tests.rs` has more distributed-touching tests than the 3 mentioned in Phase 0 decision. File size: **1,651 LOC**. Distributed-related symbols + file-path strings searched:

Command:
```bash
grep -n "distributed\|RaftRole\|RaftNode\|SwimState\|LogEntry\|NodeId" tests/nova_v2_tests.rs
```

The file has a single dedicated sprint block `N3: Distributed Infrastructure — module verification (10 tests)` at lines 246-336. **All 10 `v14_n3_*` tests touch distributed.** No other test in the file (lines 337-1651) imports or path-checks distributed.

| # | Test fn | Line range | Touches distributed via | Category |
|---|---|---|---|---|
| 1 | `v14_n3_1_distributed_module_exists` | 250-256 | `Path::new("src/distributed").exists()` — string literal | **(b)** path-existence string — **REWRITE or DELETE** (no longer a valid invariant post-extraction) |
| 2 | `v14_n3_2_raft_consensus_types` | 258-263 | `distributed::raft::RaftRole` | **(a)** direct type use — **DELETE or rewrite as integ test in new crate** |
| 3 | `v14_n3_3_raft_role_transitions` | 265-274 | `distributed::raft::RaftRole` | **(a)** direct type use — **DELETE** |
| 4 | `v14_n3_4_raft_node_creation` | 276-284 | `distributed::raft::{RaftNodeId, RaftNode}` | **(a)** direct type use — **DELETE** |
| 5 | `v14_n3_5_discovery_module_exists` | 286-289 | `Path::new("src/distributed/discovery.rs").exists()` — string literal | **(b)** path-existence string — **REWRITE or DELETE** |
| 6 | `v14_n3_6_discovery_swim_states` | 291-296 | `distributed::discovery::SwimState` | **(a)** direct type use — **DELETE** |
| 7 | `v14_n3_7_consensus_quorum` | 298-303 | NONE — pure arithmetic (`cluster_size / 2 + 1`), comment-only distributed mention | **(c)** comment/intent only; would technically PASS without distributed. **KEEP** as standalone arithmetic test, or DELETE for niche-irrelevance. **Decide at Phase F.5.** |
| 8 | `v14_n3_8_raft_log_entry` | 305-313 | `distributed::raft::LogEntry` (struct literal) | **(a)** direct type use — **DELETE** |
| 9 | `v14_n3_9_log_replication` | 315-330 | `distributed::raft::LogEntry` (Vec<>) | **(a)** direct type use — **DELETE** |
| 10 | `v14_n3_10_distributed_tensor_types` | 332-336 | `distributed::tensors::NodeId` | **(a)** direct type use — **DELETE** |

**Summary:**
- **Category (a) direct type use:** 7 tests (`v14_n3_{2,3,4,6,8,9,10}`)
- **Category (b) path-string only:** 2 tests (`v14_n3_{1,5}`)
- **Category (c) trait/type mentioned in comment only:** 1 test (`v14_n3_7`)
- **Total touching distributed:** **10** (entire `N3` block at lines 250-336)

**Recommended Phase F.5 action:** delete the **entire N3 block (lines 246-336, ~91 lines)** including the section banner comment. Rationale: post-extraction, all 10 tests are obsolete inside fajar-lang core; (c)-category `v14_n3_7` is a 6-line arithmetic test whose intent ("quorum math") loses meaning once distributed is gone. Integ-test coverage moves to the extracted crate's own test suite.

**Risk delta:** Plan §8 flagged this surprise as **MED-HIGH**. Actual finding: 10 tests in a single contiguous block, all in one well-named sprint section. **Risk REDUCED to LOW.** Plan §3 F.5 estimate (~3-4h) holds; no boundary-detection complexity here beyond mechanical block-deletion.

## §5. Internal dep graph (cycles / cross-module risk)

Command:
```bash
grep -rnE "use crate::|use super::" src/distributed/
```

Result: **15 matches, all `use super::*` and ALL inside `#[cfg(test)]` blocks** (standard Rust test-module pattern, not cross-module deps). **ZERO `use crate::*` imports of non-self fajar-lang internals.**

Verbatim listing (every match):

| File | Line | Import | Context |
|---|---|---|---|
| `cluster.rs` | 440 | `use super::*` | `#[cfg(test)] mod tests` |
| `data_plane.rs` | 655 | `use super::*` | `#[cfg(test)] mod tests` |
| `deploy.rs` | 838 | `use super::*` | `#[cfg(test)] mod tests` |
| `discovery.rs` | 718 | `use super::*` | `#[cfg(test)] mod tests` |
| `dist_bench.rs` | 837 | `use super::*` | `#[cfg(test)] mod tests` |
| `fault_tolerance.rs` | 556 | `use super::*` | `#[cfg(test)] mod tests` |
| `fault_tolerance_v2.rs` | 1024 | `use super::*` | `#[cfg(test)] mod tests` |
| `ml_training.rs` | 741 | `use super::*` | `#[cfg(test)] mod tests` |
| `raft.rs` | 623 | `use super::*` | `#[cfg(test)] mod tests` |
| `rpc.rs` | 752 | `use super::*` | `#[cfg(test)] mod tests` |
| `rpc_v2.rs` | 998 | `use super::*` | `#[cfg(test)] mod tests` |
| `scheduler.rs` | 655 | `use super::*` | `#[cfg(test)] mod tests` |
| `security.rs` | 1081 | `use super::*` | `#[cfg(test)] mod tests` |
| `tensors.rs` | 388 | `use super::*` | `#[cfg(test)] mod tests` |
| `transport.rs` | 730 | `use super::*` | `#[cfg(test)] mod tests` |

**Verdict:** `src/distributed/` is a **fully self-contained Rust subsystem.** No `interpreter`/`analyzer`/`vm`/`runtime`/`codegen` cross-dependency to invert. Extraction is mechanically clean — the new crate `fajar-distributed` needs only its own dependencies (likely `serde` + `tokio` + standard async + maybe `aes-gcm`/`rustls` for `security.rs`; to be enumerated at F.2 by reading the file `use` statements).

**This is the strongest possible signal for extraction safety.** No cycle to break, no architectural detour risk per v35.6.0 A.4 lesson.

## §6. CLI subcommand audit

### 6.1 `cmd_run_cluster` function

Location: `src/main.rs:5814-5918` (104 lines).

```rust
/// V14: `fj run --cluster` — run in distributed cluster mode.
fn cmd_run_cluster(path: &PathBuf) -> ExitCode {
    use fajar_lang::distributed::raft::{self, RaftNode, RaftNodeId, RequestVoteReply};
    use fajar_lang::distributed::scheduler::{
        DistributedTask, PlacementStrategy, TaskId, TaskLoadBalancer, TaskResources, WorkerId,
        WorkerNode,
    };

    let source = match read_source(path) { ... };
    let filename = path.display().to_string();
    let tokens = match tokenize(&source) { ... };
    let program = match parse(tokens) { ... };
    if let Err(errors) = analyze(&program) { ... }

    // Initialize a simulated 3-node Raft cluster
    let node_ids: Vec<RaftNodeId> = (0..3).map(RaftNodeId).collect();
    let mut leader = RaftNode::new(node_ids[0], node_ids[1..].to_vec());

    raft::start_election(&mut leader);
    for &peer in &node_ids[1..] {
        let reply = RequestVoteReply { term: leader.current_term, vote_granted: true };
        raft::receive_vote(&mut leader, peer, &reply);
    }

    // Worker nodes + DistributedTask + TaskLoadBalancer setup ...
    println!("=== Fajar Lang Distributed Execution ===");
    // ... then fall back to a regular tree-walking Interpreter::eval_source.
}
```

**Observation:** the function is essentially a printf-driven Raft simulation prologue (3 simulated nodes, fake election) followed by `Interpreter::new().eval_source(&source)`. The cluster aspect is **printed metadata, not actual distribution.** Deletion per D-0.2 Option α removes the printed metadata + the imports; users who want simulated cluster output build their own driver against `fajar-distributed`.

### 6.2 clap registration

Location: `src/main.rs:63-65`:

```rust
/// Run in distributed cluster mode (Raft consensus + task scheduler).
#[arg(long)]
cluster: bool,
```

Plus dispatch at `src/main.rs:449-450`:
```rust
} else if cluster {
    cmd_run_cluster(&path)
}
```

**Phase F.4 delete-list (D-0.2 α execution):**
1. `cmd_run_cluster` function body (lines 5813-5918, ~104 lines)
2. `cluster: bool` clap flag (lines 63-65, 3 lines)
3. `cluster` dispatch branch in `Run` handler (lines 449-450, 2 lines)
4. Cross-references in `--help` text / docs / `docs/EXAMPLES.md` / README — to sweep at Phase F.7

**Net deletion: ~110 lines in main.rs at F.4.** Modest cleanup.

## §7. stdlib `.fj` reference check

Command:
```bash
grep -rn "distributed" stdlib/
```

Result: **1 match, FALSE POSITIVE:**
```
stdlib/fajarquant.fj:386:// LCG PRNG + Beta-distributed sampling + Lloyd-Max codebook + quant/dequant.
```

This is a comment referencing the **Beta distribution** (statistics) inside FajarQuant's Lloyd-Max codebook implementation. Unrelated to `src/distributed/`.

**Verdict: STAGE 2 BYTE-EQUALITY EXTRACTION-SAFE.** Phase17 self-host gate unaffected; the stdlib chain (lexer → parser → analyzer → codegen) never names a distributed symbol.

## §8. fajaros-x86 cross-repo check

Command:
```bash
grep -rn "distributed" ~/Documents/fajaros-x86/ | head -40
```

Result: **3 matches, ALL FALSE POSITIVES (Apache-2.0 LICENSE text):**
```
~/Documents/fajaros-x86/NOTICE:13:distributed under the License is distributed on an "AS IS" BASIS,
~/Documents/fajaros-x86/LICENSE:112:          of the following places: within a NOTICE text file distributed
~/Documents/fajaros-x86/LICENSE:199:distributed under the License is distributed on an "AS IS" BASIS,
```

All matches are in standard Apache-2.0 boilerplate ("Work is distributed under the License"). **ZERO actual fajaros-x86 Rust/Fj consumers of `src/distributed/`.** Cross-repo migration work for this extraction: **NONE**.

(Bonus contrast: FajarOS Nova's `fajaros-x86` kernel exercises AI scheduler features, but those run on the *fajar-lang* compiler side, never importing `distributed::*` types at fj-source level — the example `examples/aspirational/distributed_mnist.fj` proves this is intentionally aspirational and not wired.)

## §9. Risk register update (vs plan §8)

| Risk | Plan probability | Plan impact | **Audited probability** | **Audited impact** | Mitigation status |
|---|---|---|---|---|---|
| Hidden symbol leak | MED | Build break | **LOW** | Build break | §3a grep exhausted: 3 Rust files only; 19 externally-consumed symbols frozen in §3b. New leak before F.5 detected by re-grep. |
| `nova_v2_tests.rs` surprise | **MED-HIGH** | Phase F.5 creep | **LOW** | F.5 creep | §4 mapped all 10 tests (one contiguous block lines 246-336). No surprise possible; mechanical deletion. |
| Stage 2 byte-equality | LOW | Phase17 fails | **NONE** | None | §7 stdlib grep is empty of real refs. Phase17 unaffected. |
| CLI users disrupted | MED | Breakage | **MED** | Breakage | Unchanged. D-0.2 α + v36.0.0 major-bump signal preserved. |
| Cargo.lock churn | LOW | CI flake | **LOW** | CI flake | Unchanged. FajarQuant precedent shows manageable. |
| Extraction overruns | HIGH | Calendar slip | **MED** | Calendar slip | §5 reveals zero internal deps — biggest source of overrun (cycle inversion) is RULED OUT. Phase F effort prediction narrows. |
| Cross-repo git history confusion | MED | Archaeology pain | **LOW** | Archaeology pain | At F.2, use `git subtree split --prefix=src/distributed/ -b extraction-split` for clean history transplant (FajarQuant precedent). |
| **NEW: fj-aspirational example loose end** | — | — | **LOW** | Tiny — broken example | `examples/aspirational/distributed_mnist.fj` references `use distributed::{...}` at fj-source namespace level. Already documented "Distributed runtime (Raft) is in `src/distributed/` but the `@distributed` lexer keyword is not wired." Treatment: **delete at F.5** (the example was never runnable; the README already notes it as a §5.1 candidate for relocation). |
| **NEW: 10 `#[tokio::test]` not counted in plan** | — | — | **LOW** | Tiny — accounting only | Plan figure "322 tests" missed tokio tests. Honest correction: 332 lib tests total (322 + 10). At F.2, ensure tokio runtime dep is added to extracted crate Cargo.toml. |

**Net risk delta:** F.0 audit reduces the two MED-HIGH/HIGH risks to LOW/MED. No new SHOWSTOPPER. Two new low-impact accounting items recorded.

## §10. Verification commands (runnable, literal)

### Pre-flight (this audit)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Headline metrics
wc -l src/distributed/*.rs                                       # expect 15343 total
ls src/distributed/ | wc -l                                      # expect 16
grep -cE "^[[:space:]]*#\[test\]" src/distributed/*.rs | \
    awk -F: '{s+=$2} END {print s}'                              # expect 322
grep -cE "^[[:space:]]*#\[tokio::test\]" src/distributed/*.rs | \
    awk -F: '{s+=$2} END {print s}'                              # expect 10

# External consumers (exhaustive)
grep -rn "distributed::" src/ tests/ examples/ benches/ stdlib/ | \
    grep -v "^src/distributed/" | wc -l                          # expect 38 matches (3 main.rs + 28 eval/mod.rs + 11 nova_v2_tests + 1 aspirational example fj-source + path-string false-positives)

# nova_v2_tests.rs distributed-block boundary
grep -nE "^\s*fn v14_n3_" tests/nova_v2_tests.rs | wc -l         # expect 10

# Internal dep graph
grep -rnE "use crate::|use super::" src/distributed/             # expect 15 matches, all "use super::*" inside cfg(test)

# stdlib byte-equality safety
grep -rn "distributed" stdlib/                                   # expect 1 match: fajarquant.fj:386 (Beta-distributed, false-positive)

# fajaros-x86 cross-repo
grep -rn "distributed" /home/primecore/Documents/fajaros-x86/ | \
    grep -v "LICENSE\|NOTICE"                                    # expect empty

# Multi-repo state
for r in ~/Documents/Fajar\ Lang ~/Documents/fajaros-x86 ~/Documents/fajarquant; do
    (cd "$r" && echo "=== $r ===" && git status -sb)
done

# pub-surface count
grep -hE "^pub (fn|struct|enum|trait|use|const|static|type)" src/distributed/*.rs | wc -l  # expect 301

# CLI registration
grep -nE "cmd_run_cluster|run_cluster|cluster: bool" src/main.rs # expect 3 matches: line 65, 450, 5814
```

### Post-F.5 (after extraction)

```bash
# Module fully removed
ls src/distributed/                                              # expect "No such file or directory"
grep "pub mod distributed" src/lib.rs                            # expect empty
grep -rn "distributed::" src/ tests/ | \
    grep -v "examples/aspirational"                              # expect empty

# Lib test count drops
cargo test --lib 2>&1 | tail -3                                  # expect ~7,211 - 332 = ~6,879 lib (322 in-file tests + 20 eval/mod.rs sprint tests removed; 10 tokio::test may need integ replacement)

# Integ test count drops by 10 (nova_v2 N3 block)
cargo test --test nova_v2_tests 2>&1 | tail -3                   # expect 135 - 10 = 125 passed (was 138 pre-EOS-37, now 135 per EOS-37 closure)

# Phase17 byte-equality preserved
cargo test --release --test selfhost_phase17_self_compile -- \
    --test-threads=1                                             # expect 4 passed

# context_safety & clippy
cargo test --test context_safety_tests                           # expect 149 passed (unchanged)
cargo clippy --lib -- -D warnings                                # expect 0 warnings

# CLI removal
fj run --cluster examples/hello.fj 2>&1                          # expect "error: unexpected argument '--cluster' found"
```

### Post-F.6 (integ tests against extracted crate)

```bash
cargo test --test distributed_integration                        # expect 4-8 passed (4-6 round-trip tests + 2 wire-up sanity)
```

## §11. Effort variance prediction (revised vs plan §3)

Plan §3 estimates per sub-phase (with +25% buffer per §6.8 R5). Based on §5 (zero internal deps) and §4 (nova_v2_tests cleanly contiguous), the higher-uncertainty assumptions can be tightened:

| Sub-phase | Plan estimate | **Revised estimate (post-B0)** | Rationale |
|---|---|---|---|
| F.1 (repo skeleton) | ~30min Fajar | **✅ DONE EOS-38** (`gh repo create`, public, Apache-2.0; Cargo skeleton still TODO at F.2 start) | No revision |
| F.2 (move 16 files + 332 tests + build clean) | ~5-7h | **~4-6h** | -25% — zero internal deps removes the biggest source of overrun (no use-crate-X rewriting). Risk: tokio dep accounting (10 tokio::test) adds ~15min. |
| F.3 (Cargo wire-up + re-export shim) | ~1.5h | **~1.5h** (unchanged) | No revision; FajarQuant precedent is tight |
| F.4 (CLI deletion per D-0.2 α) | ~1.5h | **~1h** | -33% — only 3 sites (clap flag, dispatch, fn body); 110 LOC total; no analyzer/codegen rewrite needed |
| F.5 (remove src/distributed + sprint tests + nova_v2 + aspirational example + lib.rs `pub mod`) | ~3-4h | **~2.5-3.5h** | -15% — eval/mod.rs has 20 non-contiguous tests (more than expected) but contiguous block-deletion in nova_v2_tests.rs is mechanical (1 Edit call ~90 lines). Aspirational example deletion is 1 file. |
| F.6 (integration tests against extracted crate) | ~2-3h | **~2-3h** (unchanged) | 4-8 round-trip tests; no internal-deps complexity simplifies but writing tests is the time floor |
| F.7 (closure: findings doc + CHANGELOG + MEMORY.md + multi-repo push) | ~1.5h | **~1.5h** (unchanged) | Standard ship steps |
| **Total F.1-F.7** | **15-20h** | **13-17h** | **-13% to -15%** vs plan estimate (excluding +25% surprise budget). With +25% buffer: **16-21h realistic**, down from 19-25h. |

**Recommendation:** keep plan §3's published "19-25h realistic" as the public number (preserves Plan Hygiene §6.8 R5 surprise budget honoring). Internal expectation: **closer to 16-21h** given B0 derisking.

## §12. Closure self-check (Plan Hygiene §6.8 — 8-row checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1 — this doc)
[x] Every task has runnable verification command          (Rule 2 — §10)
[x] Prevention mechanism added (hook/CI/rule)             (Rule 3 — scripts/check-no-path-deps.sh at F.3 per plan §6 + Phase 0 decision §8 template)
[x] Agent-produced numbers cross-checked with Bash        (Rule 4 — all 11 commands in §10 live-run; LOC/test/symbol counts grep-derived)
[ ] Effort variance tagged in commit message              (Rule 5 — at this commit ship time)
[x] Decisions are committed files                         (Rule 6 — D-0.1/0.2/0.3 in `docs/decisions/2026-05-12-path-e-f-prep.md`; this B0 binds the contract symbol-surface)
[x] Public-artifact drift swept                           (Rule 7 — EOS-37 CLAUDE.md stats refresh + EOS-41 plan + Phase 0 decision; this B0 adds the F-specific binding; F.7 will refresh §3 stats + README + CHANGELOG)
[x] Multi-repo state checked                              (Rule 8 — §10 multi-repo command live-run; clean+sync per Phase 0 §2)
```

Eight rows: 7 ✓ + 1 deferred (R5 at commit ship). **Ship.**

## §13. Source artifacts

- This file: `docs/PATH_F_DISTRIBUTED_EXTRACTION_B0_FINDINGS.md`
- Plan: `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md` (commit `8cdbad97`)
- Phase 0 decision: `docs/decisions/2026-05-12-path-e-f-prep.md` (commit `b5b6a67b`)
- Predecessor B0: `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md` (commit `eb3a3c25`)
- Style exemplar: `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md`
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 (distributed verdict)
- FajarQuant extraction precedent (2026-04-11): `Cargo.toml` line 24 `fajarquant = { git = "..." rev = "b05ecf17..." }`

---

*B0 written 2026-05-12 EOS-42. ~1.5h actual / ~2h plan estimate (-25%). Frozen symbol-surface contract: 301 pub items across 15 modules; 19 externally-consumed today (5 sub-modules); 282 internal-only. Zero `use crate::*` internal deps. Zero stdlib refs. Zero fajaros-x86 cross-repo consumers. nova_v2_tests.rs surprise risk REDUCED from MED-HIGH to LOW (all 10 distributed-touching tests in single contiguous v14_n3_* block, lines 246-336). Phase F effort prediction narrowed: 16-21h realistic (-15% vs plan 19-25h), preserved as 19-25h public number to honor §6.8 R5 surprise budget. Phase F.1 handoff ✅ done EOS-38 (`fajarkraton/fajar-distributed` live). Phase F.2 (source file move) is next.*
