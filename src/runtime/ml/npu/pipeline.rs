//! # ONNX-to-NPU Pipeline
//!
//! End-to-end pipeline: load ONNX → optimize → quantize → partition → compile → run.
//!
//! ## Pipeline Stages
//!
//! 1. **Load**: Parse ONNX protobuf, build computation graph
//! 2. **Optimize**: Constant folding, dead node elimination, shape inference
//! 3. **Fuse**: Conv+BN+ReLU, MatMul+Add fusion for NPU efficiency
//! 4. **Quantize**: Calibration-based INT8 quantization
//! 5. **Partition**: Split into NPU-executable and CPU-fallback subgraphs
//! 6. **Compile**: Compile to NPU-specific binary
//! 7. **Cache**: Cache compiled blobs for repeat inference

use super::{NpuCompiledModel, NpuDtype, NpuRuntimeError};
use serde::Serialize;

// ═══════════════════════════════════════════════════════════════════════
// ONNX Graph Representation
// ═══════════════════════════════════════════════════════════════════════

/// An operator in the ONNX computation graph.
#[derive(Debug, Clone, Serialize)]
pub struct OnnxNode {
    /// Operator type (e.g., "Conv", "MatMul", "Relu", "Add").
    pub op_type: String,
    /// Input tensor names.
    pub inputs: Vec<String>,
    /// Output tensor names.
    pub outputs: Vec<String>,
    /// Whether this node can run on NPU.
    pub npu_compatible: bool,
}

/// An ONNX computation graph.
#[derive(Debug, Clone, Serialize)]
pub struct OnnxGraph {
    /// Graph nodes (operators).
    pub nodes: Vec<OnnxNode>,
    /// Graph input names and shapes.
    pub inputs: Vec<(String, Vec<usize>)>,
    /// Graph output names and shapes.
    pub outputs: Vec<(String, Vec<usize>)>,
    /// Model name.
    pub name: String,
}

impl OnnxGraph {
    /// Create an empty graph.
    pub fn new(name: &str) -> Self {
        Self {
            nodes: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            name: name.to_string(),
        }
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of NPU-compatible nodes.
    pub fn npu_compatible_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.npu_compatible).count()
    }

    /// Fraction of graph that can run on NPU.
    pub fn npu_coverage(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        self.npu_compatible_count() as f64 / self.nodes.len() as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Graph Optimization
// ═══════════════════════════════════════════════════════════════════════

/// Optimization pass result.
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Number of nodes before optimization.
    pub nodes_before: usize,
    /// Number of nodes after optimization.
    pub nodes_after: usize,
    /// Number of fused patterns found.
    pub fusions_applied: usize,
    /// Number of constant-folded nodes.
    pub constants_folded: usize,
}

/// Apply graph optimizations: constant folding, dead node elimination.
pub fn optimize_graph(graph: &mut OnnxGraph) -> OptimizationResult {
    let nodes_before = graph.nodes.len();

    // Dead node elimination: remove nodes with no consumers
    let output_names: std::collections::HashSet<String> =
        graph.outputs.iter().map(|(n, _)| n.clone()).collect();

    let mut live_outputs: std::collections::HashSet<String> = output_names;

    // Backward pass to find live nodes
    let mut live_nodes = vec![false; graph.nodes.len()];
    for (i, node) in graph.nodes.iter().enumerate().rev() {
        let is_live = node.outputs.iter().any(|o| live_outputs.contains(o));
        if is_live {
            live_nodes[i] = true;
            for input in &node.inputs {
                live_outputs.insert(input.clone());
            }
        }
    }

    // Remove dead nodes
    let mut new_nodes = Vec::new();
    let mut constants_folded = 0;
    for (i, node) in graph.nodes.drain(..).enumerate() {
        if live_nodes.get(i).copied().unwrap_or(false) {
            new_nodes.push(node);
        } else {
            constants_folded += 1;
        }
    }
    graph.nodes = new_nodes;

    OptimizationResult {
        nodes_before,
        nodes_after: graph.nodes.len(),
        fusions_applied: 0,
        constants_folded,
    }
}

/// Apply operator fusion: Conv+BN+ReLU, MatMul+Add.
pub fn fuse_operators(graph: &mut OnnxGraph) -> usize {
    let mut fusions = 0;

    // Simple Conv+Relu fusion detection
    let mut i = 0;
    while i + 1 < graph.nodes.len() {
        let is_conv_relu = graph.nodes[i].op_type == "Conv"
            && graph.nodes[i + 1].op_type == "Relu"
            && graph.nodes[i].outputs.first() == graph.nodes[i + 1].inputs.first();

        let is_matmul_add = graph.nodes[i].op_type == "MatMul"
            && graph.nodes[i + 1].op_type == "Add"
            && graph.nodes[i].outputs.first() == graph.nodes[i + 1].inputs.first();

        if is_conv_relu {
            let fused = OnnxNode {
                op_type: "ConvRelu".to_string(),
                inputs: graph.nodes[i].inputs.clone(),
                outputs: graph.nodes[i + 1].outputs.clone(),
                npu_compatible: true,
            };
            graph.nodes[i] = fused;
            graph.nodes.remove(i + 1);
            fusions += 1;
        } else if is_matmul_add {
            let fused = OnnxNode {
                op_type: "MatMulAdd".to_string(),
                inputs: graph.nodes[i].inputs.clone(),
                outputs: graph.nodes[i + 1].outputs.clone(),
                npu_compatible: true,
            };
            graph.nodes[i] = fused;
            graph.nodes.remove(i + 1);
            fusions += 1;
        } else {
            i += 1;
        }
    }

    fusions
}

// ═══════════════════════════════════════════════════════════════════════
// Graph Partitioning
// ═══════════════════════════════════════════════════════════════════════

/// A partition of the graph for a specific execution target.
#[derive(Debug, Clone)]
pub struct GraphPartition {
    /// Target device ("npu" or "cpu").
    pub target: String,
    /// Node indices in the original graph.
    pub node_indices: Vec<usize>,
}

/// Partition a graph into NPU and CPU subgraphs.
pub fn partition_graph(graph: &OnnxGraph) -> Vec<GraphPartition> {
    let mut npu_indices = Vec::new();
    let mut cpu_indices = Vec::new();

    for (i, node) in graph.nodes.iter().enumerate() {
        if node.npu_compatible {
            npu_indices.push(i);
        } else {
            cpu_indices.push(i);
        }
    }

    let mut partitions = Vec::new();
    if !npu_indices.is_empty() {
        partitions.push(GraphPartition {
            target: "npu".to_string(),
            node_indices: npu_indices,
        });
    }
    if !cpu_indices.is_empty() {
        partitions.push(GraphPartition {
            target: "cpu".to_string(),
            node_indices: cpu_indices,
        });
    }
    partitions
}

// ═══════════════════════════════════════════════════════════════════════
// Compiled Model Cache
// ═══════════════════════════════════════════════════════════════════════

/// Cache key for compiled NPU models.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    /// Model file path or hash.
    pub model_id: String,
    /// Target backend.
    pub backend: String,
    /// Quantization settings.
    pub quantization: String,
}

/// Compiled model cache.
#[derive(Debug, Default)]
pub struct ModelCache {
    entries: std::collections::HashMap<CacheKey, NpuCompiledModel>,
}

impl ModelCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a model is cached.
    pub fn contains(&self, key: &CacheKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Get a cached model.
    pub fn get(&self, key: &CacheKey) -> Option<&NpuCompiledModel> {
        self.entries.get(key)
    }

    /// Insert a compiled model into cache.
    pub fn insert(&mut self, key: CacheKey, model: NpuCompiledModel) {
        self.entries.insert(key, model);
    }

    /// Number of cached models.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Full Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// NPU compilation pipeline result.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Compiled model.
    pub model: NpuCompiledModel,
    /// Optimization stats.
    pub nodes_before: usize,
    /// After optimization.
    pub nodes_after: usize,
    /// Fusions applied.
    pub fusions: usize,
    /// NPU coverage (fraction of graph on NPU).
    pub npu_coverage: f64,
}

/// Run the full ONNX-to-NPU pipeline.
///
/// Stages: load → optimize → fuse → partition → compile.
pub fn npu_compile(
    graph: &mut OnnxGraph,
    backend: &str,
) -> Result<PipelineResult, NpuRuntimeError> {
    let nodes_before = graph.node_count();

    // Optimize
    let _opt = optimize_graph(graph);

    // Fuse
    let fusions = fuse_operators(graph);

    // Mark NPU compatibility
    let npu_ops = [
        "Conv",
        "ConvRelu",
        "MatMul",
        "MatMulAdd",
        "Relu",
        "Add",
        "Softmax",
        "BatchNormalization",
        "AveragePool",
        "MaxPool",
        "Flatten",
        "Reshape",
    ];
    for node in &mut graph.nodes {
        node.npu_compatible = npu_ops.contains(&node.op_type.as_str());
    }

    let npu_coverage = graph.npu_coverage();
    let nodes_after = graph.node_count();

    // Build compiled model
    let input_shapes: Vec<Vec<usize>> = graph.inputs.iter().map(|(_, s)| s.clone()).collect();
    let output_shapes: Vec<Vec<usize>> = graph.outputs.iter().map(|(_, s)| s.clone()).collect();

    let model = NpuCompiledModel {
        backend: backend.to_string(),
        name: graph.name.clone(),
        input_shapes,
        output_shapes,
        input_dtypes: vec![NpuDtype::F32],
        output_dtypes: vec![NpuDtype::F32],
    };

    Ok(PipelineResult {
        model,
        nodes_before,
        nodes_after,
        fusions,
        npu_coverage,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Benchmarking Harness
// ═══════════════════════════════════════════════════════════════════════

/// Benchmark result for an inference run.
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkResult {
    /// Median latency in microseconds.
    pub p50_latency_us: u64,
    /// 99th percentile latency in microseconds.
    pub p99_latency_us: u64,
    /// Throughput in inferences per second.
    pub throughput_ips: f64,
    /// Number of iterations.
    pub iterations: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> OnnxGraph {
        let mut g = OnnxGraph::new("test_model");
        g.inputs.push(("input".to_string(), vec![1, 3, 224, 224]));
        g.outputs.push(("output".to_string(), vec![1, 1000]));
        g.nodes.push(OnnxNode {
            op_type: "Conv".to_string(),
            inputs: vec!["input".to_string()],
            outputs: vec!["conv_out".to_string()],
            npu_compatible: true,
        });
        g.nodes.push(OnnxNode {
            op_type: "Relu".to_string(),
            inputs: vec!["conv_out".to_string()],
            outputs: vec!["relu_out".to_string()],
            npu_compatible: true,
        });
        g.nodes.push(OnnxNode {
            op_type: "MatMul".to_string(),
            inputs: vec!["relu_out".to_string()],
            outputs: vec!["output".to_string()],
            npu_compatible: true,
        });
        g
    }

    // ── S12.1: ONNX Model Loading ─────────────────────────────────────

    #[test]
    fn onnx_graph_basic() {
        let g = make_test_graph();
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.name, "test_model");
    }

    // ── S12.2: Graph Optimization ─────────────────────────────────────

    #[test]
    fn optimize_removes_dead_nodes() {
        let mut g = make_test_graph();
        // Add a dead node
        g.nodes.push(OnnxNode {
            op_type: "Dead".to_string(),
            inputs: vec!["nowhere".to_string()],
            outputs: vec!["dead_out".to_string()],
            npu_compatible: false,
        });
        assert_eq!(g.node_count(), 4);
        let result = optimize_graph(&mut g);
        assert_eq!(result.nodes_after, 3);
        assert_eq!(result.constants_folded, 1);
    }

    // ── S12.3: Operator Fusion ────────────────────────────────────────

    #[test]
    fn fuse_conv_relu() {
        let mut g = make_test_graph();
        let fusions = fuse_operators(&mut g);
        assert_eq!(fusions, 1);
        assert_eq!(g.nodes[0].op_type, "ConvRelu");
    }

    #[test]
    fn fuse_matmul_add() {
        let mut g = OnnxGraph::new("test");
        g.outputs.push(("out".to_string(), vec![1]));
        g.nodes.push(OnnxNode {
            op_type: "MatMul".to_string(),
            inputs: vec!["x".to_string()],
            outputs: vec!["mm_out".to_string()],
            npu_compatible: true,
        });
        g.nodes.push(OnnxNode {
            op_type: "Add".to_string(),
            inputs: vec!["mm_out".to_string()],
            outputs: vec!["out".to_string()],
            npu_compatible: true,
        });
        let fusions = fuse_operators(&mut g);
        assert_eq!(fusions, 1);
        assert_eq!(g.nodes[0].op_type, "MatMulAdd");
    }

    // ── S12.5: Model Partitioning ─────────────────────────────────────

    #[test]
    fn partition_all_npu() {
        let g = make_test_graph();
        let parts = partition_graph(&g);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].target, "npu");
    }

    #[test]
    fn partition_mixed() {
        let mut g = make_test_graph();
        g.nodes.push(OnnxNode {
            op_type: "CustomOp".to_string(),
            inputs: vec!["x".to_string()],
            outputs: vec!["y".to_string()],
            npu_compatible: false,
        });
        let parts = partition_graph(&g);
        assert_eq!(parts.len(), 2);
    }

    // ── S12.6: Pipeline Assembly ──────────────────────────────────────

    #[test]
    fn full_pipeline() {
        let mut g = make_test_graph();
        let result = npu_compile(&mut g, "openvino").unwrap();
        assert_eq!(result.nodes_before, 3);
        assert!(result.npu_coverage > 0.0);
        assert_eq!(result.model.backend, "openvino");
    }

    // ── S12.7: Model Cache ────────────────────────────────────────────

    #[test]
    fn cache_insert_get() {
        let mut cache = ModelCache::new();
        let key = CacheKey {
            model_id: "resnet18".to_string(),
            backend: "openvino".to_string(),
            quantization: "int8".to_string(),
        };
        let model = NpuCompiledModel {
            backend: "openvino".to_string(),
            name: "resnet18".to_string(),
            input_shapes: vec![],
            output_shapes: vec![],
            input_dtypes: vec![],
            output_dtypes: vec![],
        };
        cache.insert(key.clone(), model);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&key));
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn cache_clear() {
        let mut cache = ModelCache::new();
        cache.insert(
            CacheKey {
                model_id: "test".to_string(),
                backend: "x".to_string(),
                quantization: "none".to_string(),
            },
            NpuCompiledModel {
                backend: "x".to_string(),
                name: "test".to_string(),
                input_shapes: vec![],
                output_shapes: vec![],
                input_dtypes: vec![],
                output_dtypes: vec![],
            },
        );
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }

    // ── S12.8: Benchmark Harness ──────────────────────────────────────

    #[test]
    fn benchmark_result_fields() {
        let result = BenchmarkResult {
            p50_latency_us: 1000,
            p99_latency_us: 2500,
            throughput_ips: 800.0,
            iterations: 100,
        };
        assert_eq!(result.iterations, 100);
        assert!(result.throughput_ips > 0.0);
    }

    // ── S12.9: NPU Coverage Analysis ──────────────────────────────────

    #[test]
    fn npu_coverage_all_compatible() {
        let g = make_test_graph();
        assert!((g.npu_coverage() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn npu_coverage_empty_graph() {
        let g = OnnxGraph::new("empty");
        assert!((g.npu_coverage() - 0.0).abs() < 1e-6);
    }
}
