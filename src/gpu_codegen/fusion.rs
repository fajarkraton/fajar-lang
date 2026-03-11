//! Kernel Fusion — detect fuseable kernel sequences, element-wise fusion,
//! reduction fusion, memory planning, tiling, auto-tuning, fusion graph.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S19.1: Fusion Analysis
// ═══════════════════════════════════════════════════════════════════════

/// A GPU operation node in the fusion graph.
#[derive(Debug, Clone)]
pub struct GpuOp {
    /// Unique operation ID.
    pub id: usize,
    /// Operation kind.
    pub kind: OpKind,
    /// Input dependencies (IDs of ops that produce inputs).
    pub inputs: Vec<usize>,
    /// Output shape (simplified as element count).
    pub output_elements: usize,
}

/// Kind of GPU operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// Element-wise unary (relu, sigmoid, etc.).
    ElementWiseUnary,
    /// Element-wise binary (add, mul, etc.).
    ElementWiseBinary,
    /// Matrix multiplication.
    Matmul,
    /// Reduction (sum, max, mean).
    Reduction,
    /// Reshape/transpose (no computation).
    Reshape,
    /// Convolution.
    Conv,
    /// Softmax (reduction + element-wise).
    Softmax,
    /// Custom kernel.
    Custom,
}

impl fmt::Display for OpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpKind::ElementWiseUnary => write!(f, "ElementWiseUnary"),
            OpKind::ElementWiseBinary => write!(f, "ElementWiseBinary"),
            OpKind::Matmul => write!(f, "Matmul"),
            OpKind::Reduction => write!(f, "Reduction"),
            OpKind::Reshape => write!(f, "Reshape"),
            OpKind::Conv => write!(f, "Conv"),
            OpKind::Softmax => write!(f, "Softmax"),
            OpKind::Custom => write!(f, "Custom"),
        }
    }
}

/// Checks if two operations can be fused.
pub fn can_fuse(producer: &GpuOp, consumer: &GpuOp) -> bool {
    // Consumer must depend on producer
    if !consumer.inputs.contains(&producer.id) {
        return false;
    }

    match (producer.kind, consumer.kind) {
        // Element-wise chains are always fuseable
        (
            OpKind::ElementWiseUnary | OpKind::ElementWiseBinary,
            OpKind::ElementWiseUnary | OpKind::ElementWiseBinary,
        ) => true,
        // Matmul + element-wise (bias add, activation)
        (OpKind::Matmul, OpKind::ElementWiseUnary | OpKind::ElementWiseBinary) => true,
        // Element-wise + reduction
        (OpKind::ElementWiseUnary | OpKind::ElementWiseBinary, OpKind::Reduction) => true,
        // Conv + element-wise (activation)
        (OpKind::Conv, OpKind::ElementWiseUnary) => true,
        _ => false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.2: Element-wise Fusion
// ═══════════════════════════════════════════════════════════════════════

/// A fused kernel — a sequence of operations combined into one kernel.
#[derive(Debug, Clone)]
pub struct FusedKernel {
    /// Kernel name.
    pub name: String,
    /// Operations in execution order.
    pub ops: Vec<OpKind>,
    /// Original op IDs that were fused.
    pub fused_op_ids: Vec<usize>,
    /// Total elements processed.
    pub elements: usize,
}

/// Detects chains of element-wise operations and fuses them.
pub fn fuse_elementwise_chain(ops: &[GpuOp]) -> Vec<FusedKernel> {
    let mut kernels = Vec::new();
    let mut visited = vec![false; ops.len()];
    let mut kernel_counter = 0;

    for i in 0..ops.len() {
        if visited[i] {
            continue;
        }
        if !is_elementwise(ops[i].kind) {
            continue;
        }

        // Start a chain
        let mut chain = vec![i];
        visited[i] = true;

        // Extend forward
        let mut current = i;
        loop {
            let next = ops.iter().position(|op| {
                !visited[op.id] && is_elementwise(op.kind) && op.inputs.contains(&ops[current].id)
            });
            match next {
                Some(n) => {
                    chain.push(n);
                    visited[n] = true;
                    current = n;
                }
                None => break,
            }
        }

        if chain.len() > 1 {
            kernels.push(FusedKernel {
                name: format!("fused_ew_{kernel_counter}"),
                ops: chain.iter().map(|&idx| ops[idx].kind).collect(),
                fused_op_ids: chain.iter().map(|&idx| ops[idx].id).collect(),
                elements: ops[chain[0]].output_elements,
            });
            kernel_counter += 1;
        }
    }

    kernels
}

/// Checks if an op kind is element-wise.
pub fn is_elementwise(kind: OpKind) -> bool {
    matches!(kind, OpKind::ElementWiseUnary | OpKind::ElementWiseBinary)
}

// ═══════════════════════════════════════════════════════════════════════
// S19.3: Reduction Fusion
// ═══════════════════════════════════════════════════════════════════════

/// Detects element-wise ops followed by reduction and fuses them.
pub fn fuse_reduction_chain(ops: &[GpuOp]) -> Vec<FusedKernel> {
    let mut kernels = Vec::new();
    let mut kernel_counter = 0;

    for (i, op) in ops.iter().enumerate() {
        if op.kind != OpKind::Reduction {
            continue;
        }

        // Find preceding element-wise ops that feed into this reduction
        let mut chain: Vec<usize> = op
            .inputs
            .iter()
            .filter_map(|&input_id| {
                ops.iter()
                    .position(|o| o.id == input_id && is_elementwise(o.kind))
            })
            .collect();

        if !chain.is_empty() {
            chain.push(i);
            kernels.push(FusedKernel {
                name: format!("fused_reduce_{kernel_counter}"),
                ops: chain.iter().map(|&idx| ops[idx].kind).collect(),
                fused_op_ids: chain.iter().map(|&idx| ops[idx].id).collect(),
                elements: ops[i].output_elements,
            });
            kernel_counter += 1;
        }
    }

    kernels
}

// ═══════════════════════════════════════════════════════════════════════
// S19.4: Memory Planning
// ═══════════════════════════════════════════════════════════════════════

/// Memory allocation plan for a kernel.
#[derive(Debug, Clone)]
pub struct MemoryPlan {
    /// Allocations needed.
    pub allocations: Vec<Allocation>,
    /// Total bytes.
    pub total_bytes: usize,
    /// Savings from fusion (bytes eliminated).
    pub saved_bytes: usize,
}

/// A memory allocation.
#[derive(Debug, Clone)]
pub struct Allocation {
    /// Buffer name.
    pub name: String,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Whether this is an intermediate (can be eliminated by fusion).
    pub is_intermediate: bool,
}

/// Plans memory for a fused kernel, eliminating intermediate buffers.
pub fn plan_memory(fused: &FusedKernel, elem_size_bytes: usize) -> MemoryPlan {
    let total_elements = fused.elements;
    let input_alloc = Allocation {
        name: format!("{}_input", fused.name),
        size_bytes: total_elements * elem_size_bytes,
        is_intermediate: false,
    };
    let output_alloc = Allocation {
        name: format!("{}_output", fused.name),
        size_bytes: total_elements * elem_size_bytes,
        is_intermediate: false,
    };

    // Intermediates that are eliminated
    let num_intermediates = if fused.ops.len() > 1 {
        fused.ops.len() - 1
    } else {
        0
    };
    let saved = num_intermediates * total_elements * elem_size_bytes;

    MemoryPlan {
        total_bytes: input_alloc.size_bytes + output_alloc.size_bytes,
        allocations: vec![input_alloc, output_alloc],
        saved_bytes: saved,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.5: Tiling Strategy
// ═══════════════════════════════════════════════════════════════════════

/// A tiling configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileConfig {
    /// Tile size in X.
    pub tile_x: u32,
    /// Tile size in Y.
    pub tile_y: u32,
    /// Elements per thread.
    pub elements_per_thread: u32,
}

/// Standard tile sizes for different operation types.
pub fn default_tile_config(kind: OpKind) -> TileConfig {
    match kind {
        OpKind::Matmul => TileConfig {
            tile_x: 16,
            tile_y: 16,
            elements_per_thread: 4,
        },
        OpKind::Conv => TileConfig {
            tile_x: 8,
            tile_y: 8,
            elements_per_thread: 2,
        },
        OpKind::Reduction => TileConfig {
            tile_x: 256,
            tile_y: 1,
            elements_per_thread: 4,
        },
        _ => TileConfig {
            tile_x: 256,
            tile_y: 1,
            elements_per_thread: 1,
        },
    }
}

/// Computes shared memory usage for a tiled kernel.
pub fn shared_memory_bytes(tile: TileConfig, elem_size: u32) -> u32 {
    tile.tile_x * tile.tile_y * tile.elements_per_thread * elem_size
}

// ═══════════════════════════════════════════════════════════════════════
// S19.6: Auto-Tuning
// ═══════════════════════════════════════════════════════════════════════

/// An auto-tuning configuration candidate.
#[derive(Debug, Clone)]
pub struct TuningCandidate {
    /// Tile configuration.
    pub tile: TileConfig,
    /// Shared memory bytes.
    pub shared_mem_bytes: u32,
    /// Estimated occupancy (0.0 - 1.0).
    pub occupancy: f64,
    /// Measured or estimated throughput (elements/sec).
    pub throughput: f64,
}

/// Generates tuning candidates for a given operation.
pub fn generate_candidates(kind: OpKind, max_shared_mem: u32) -> Vec<TuningCandidate> {
    let tile_sizes: Vec<(u32, u32)> = match kind {
        OpKind::Matmul => vec![(8, 8), (16, 16), (32, 32)],
        OpKind::Conv => vec![(4, 4), (8, 8), (16, 16)],
        _ => vec![(64, 1), (128, 1), (256, 1), (512, 1)],
    };

    let elem_size = 4; // f32

    tile_sizes
        .into_iter()
        .filter_map(|(tx, ty)| {
            let tile = TileConfig {
                tile_x: tx,
                tile_y: ty,
                elements_per_thread: 1,
            };
            let smem = shared_memory_bytes(tile, elem_size);
            if smem > max_shared_mem {
                return None;
            }
            let threads = tx * ty;
            let occupancy = (threads as f64 / 1024.0).min(1.0);
            Some(TuningCandidate {
                tile,
                shared_mem_bytes: smem,
                occupancy,
                throughput: 0.0, // To be measured
            })
        })
        .collect()
}

/// Selects the best candidate (highest occupancy as heuristic).
pub fn select_best(candidates: &[TuningCandidate]) -> Option<&TuningCandidate> {
    candidates.iter().max_by(|a, b| {
        a.occupancy
            .partial_cmp(&b.occupancy)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S19.7: Fusion Graph
// ═══════════════════════════════════════════════════════════════════════

/// A fusion graph — dataflow graph of GPU operations.
#[derive(Debug, Clone)]
pub struct FusionGraph {
    /// All operations.
    pub ops: Vec<GpuOp>,
    /// Fusion groups (each group = list of op indices to fuse).
    pub groups: Vec<Vec<usize>>,
}

impl FusionGraph {
    /// Creates a new fusion graph from ops.
    pub fn new(ops: Vec<GpuOp>) -> Self {
        Self {
            ops,
            groups: Vec::new(),
        }
    }

    /// Analyzes the graph and populates fusion groups.
    pub fn analyze(&mut self) {
        self.groups.clear();
        let n = self.ops.len();
        let mut in_group = vec![false; n];

        // Greedily find fuseable pairs and extend into groups
        for i in 0..n {
            if in_group[i] {
                continue;
            }
            let mut group = vec![i];
            in_group[i] = true;

            // Try to extend this group with downstream consumers
            let mut last = i;
            for (j, flag) in in_group.iter_mut().enumerate().skip(i + 1) {
                if *flag {
                    continue;
                }
                if can_fuse(&self.ops[last], &self.ops[j]) {
                    group.push(j);
                    *flag = true;
                    last = j;
                }
            }

            if group.len() > 1 {
                self.groups.push(group);
            }
        }
    }

    /// Returns total number of fusion opportunities found.
    pub fn num_fusions(&self) -> usize {
        self.groups.len()
    }

    /// Returns total ops fused.
    pub fn total_fused_ops(&self) -> usize {
        self.groups.iter().map(|g| g.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.8: Fusion Limits
// ═══════════════════════════════════════════════════════════════════════

/// Fusion configuration limits.
#[derive(Debug, Clone)]
pub struct FusionLimits {
    /// Maximum ops in a single fused kernel.
    pub max_ops_per_kernel: usize,
    /// Maximum registers per thread.
    pub max_registers: u32,
    /// Maximum shared memory per block (bytes).
    pub max_shared_memory: u32,
    /// Maximum kernel instruction count.
    pub max_instructions: usize,
}

impl Default for FusionLimits {
    fn default() -> Self {
        Self {
            max_ops_per_kernel: 16,
            max_registers: 64,
            max_shared_memory: 49152, // 48 KB
            max_instructions: 4096,
        }
    }
}

impl FusionLimits {
    /// Checks if a fusion group is within limits.
    pub fn within_limits(&self, group_size: usize, shared_mem: u32) -> bool {
        group_size <= self.max_ops_per_kernel && shared_mem <= self.max_shared_memory
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S19.9: Fusion Report
// ═══════════════════════════════════════════════════════════════════════

/// A fusion report entry.
#[derive(Debug, Clone)]
pub struct FusionReportEntry {
    /// Fused kernel name.
    pub kernel_name: String,
    /// Number of ops fused.
    pub ops_fused: usize,
    /// Memory saved (bytes).
    pub memory_saved: usize,
    /// Estimated speedup factor.
    pub estimated_speedup: f64,
}

/// Generates a fusion report from a graph.
pub fn generate_report(graph: &FusionGraph, elem_size: usize) -> Vec<FusionReportEntry> {
    graph
        .groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let ops_fused = group.len();
            let intermediates_saved = ops_fused.saturating_sub(1);
            // Estimate: each intermediate is ~output_elements * elem_size
            let representative_elements =
                graph.ops.get(group[0]).map_or(0, |op| op.output_elements);
            let memory_saved = intermediates_saved * representative_elements * elem_size;
            // Heuristic: each eliminated kernel launch saves ~10us overhead
            let estimated_speedup = 1.0 + (intermediates_saved as f64 * 0.1);

            FusionReportEntry {
                kernel_name: format!("fused_kernel_{i}"),
                ops_fused,
                memory_saved,
                estimated_speedup,
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ew_ops() -> Vec<GpuOp> {
        vec![
            GpuOp {
                id: 0,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![],
                output_elements: 1024,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseBinary,
                inputs: vec![0],
                output_elements: 1024,
            },
            GpuOp {
                id: 2,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![1],
                output_elements: 1024,
            },
        ]
    }

    // S19.1 — Fusion Analysis
    #[test]
    fn s19_1_can_fuse_elementwise_chain() {
        let ops = make_ew_ops();
        assert!(can_fuse(&ops[0], &ops[1]));
        assert!(can_fuse(&ops[1], &ops[2]));
    }

    #[test]
    fn s19_1_cannot_fuse_unrelated() {
        let ops = make_ew_ops();
        assert!(!can_fuse(&ops[0], &ops[2])); // ops[2] depends on ops[1], not ops[0]
    }

    #[test]
    fn s19_1_can_fuse_matmul_activation() {
        let matmul = GpuOp {
            id: 0,
            kind: OpKind::Matmul,
            inputs: vec![],
            output_elements: 256,
        };
        let relu = GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![0],
            output_elements: 256,
        };
        assert!(can_fuse(&matmul, &relu));
    }

    // S19.2 — Element-wise Fusion
    #[test]
    fn s19_2_fuse_elementwise_chain() {
        let ops = make_ew_ops();
        let fused = fuse_elementwise_chain(&ops);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].ops.len(), 3);
        assert_eq!(fused[0].fused_op_ids, vec![0, 1, 2]);
    }

    #[test]
    fn s19_2_no_fusion_single_op() {
        let ops = vec![GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 512,
        }];
        let fused = fuse_elementwise_chain(&ops);
        assert!(fused.is_empty());
    }

    // S19.3 — Reduction Fusion
    #[test]
    fn s19_3_fuse_ew_reduction() {
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![],
                output_elements: 1024,
            },
            GpuOp {
                id: 1,
                kind: OpKind::Reduction,
                inputs: vec![0],
                output_elements: 1,
            },
        ];
        let fused = fuse_reduction_chain(&ops);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].ops.len(), 2);
    }

    // S19.4 — Memory Planning
    #[test]
    fn s19_4_memory_plan() {
        let fused = FusedKernel {
            name: "test".into(),
            ops: vec![
                OpKind::ElementWiseUnary,
                OpKind::ElementWiseBinary,
                OpKind::ElementWiseUnary,
            ],
            fused_op_ids: vec![0, 1, 2],
            elements: 1024,
        };
        let plan = plan_memory(&fused, 4);
        assert_eq!(plan.allocations.len(), 2); // input + output only
        assert_eq!(plan.saved_bytes, 2 * 1024 * 4); // 2 intermediates saved
        assert_eq!(plan.total_bytes, 2 * 1024 * 4);
    }

    // S19.5 — Tiling Strategy
    #[test]
    fn s19_5_default_matmul_tile() {
        let tile = default_tile_config(OpKind::Matmul);
        assert_eq!(tile.tile_x, 16);
        assert_eq!(tile.tile_y, 16);
    }

    #[test]
    fn s19_5_shared_memory() {
        let tile = TileConfig {
            tile_x: 16,
            tile_y: 16,
            elements_per_thread: 4,
        };
        assert_eq!(shared_memory_bytes(tile, 4), 16 * 16 * 4 * 4);
    }

    // S19.6 — Auto-Tuning
    #[test]
    fn s19_6_generate_candidates() {
        let candidates = generate_candidates(OpKind::Matmul, 49152);
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|c| c.shared_mem_bytes <= 49152));
    }

    #[test]
    fn s19_6_select_best() {
        let candidates = generate_candidates(OpKind::Matmul, 49152);
        let best = select_best(&candidates);
        assert!(best.is_some());
    }

    // S19.7 — Fusion Graph
    #[test]
    fn s19_7_fusion_graph_analyze() {
        let ops = make_ew_ops();
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        assert!(graph.num_fusions() >= 1);
        assert!(graph.total_fused_ops() >= 2);
    }

    // S19.8 — Fusion Limits
    #[test]
    fn s19_8_default_limits() {
        let limits = FusionLimits::default();
        assert_eq!(limits.max_ops_per_kernel, 16);
        assert_eq!(limits.max_shared_memory, 49152);
    }

    #[test]
    fn s19_8_within_limits() {
        let limits = FusionLimits::default();
        assert!(limits.within_limits(5, 4096));
        assert!(!limits.within_limits(20, 4096));
        assert!(!limits.within_limits(5, 60000));
    }

    // S19.9 — Fusion Report
    #[test]
    fn s19_9_generate_report() {
        let ops = make_ew_ops();
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        let report = generate_report(&graph, 4);
        assert!(!report.is_empty());
        assert!(report[0].ops_fused >= 2);
        assert!(report[0].memory_saved > 0);
        assert!(report[0].estimated_speedup > 1.0);
    }

    // S19.10 — Integration
    #[test]
    fn s19_10_op_kind_display() {
        assert_eq!(OpKind::Matmul.to_string(), "Matmul");
        assert_eq!(OpKind::ElementWiseUnary.to_string(), "ElementWiseUnary");
        assert_eq!(OpKind::Reduction.to_string(), "Reduction");
    }

    #[test]
    fn s19_10_is_elementwise() {
        assert!(is_elementwise(OpKind::ElementWiseUnary));
        assert!(is_elementwise(OpKind::ElementWiseBinary));
        assert!(!is_elementwise(OpKind::Matmul));
        assert!(!is_elementwise(OpKind::Reduction));
    }
}
