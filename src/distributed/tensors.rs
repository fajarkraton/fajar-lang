//! Distributed Tensors — sharding, placement, distributed matmul,
//! allreduce, ring allreduce, parameter server, data/model parallelism.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S10.1: Tensor Sharding
// ═══════════════════════════════════════════════════════════════════════

/// A unique node identifier in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({})", self.0)
    }
}

/// A shard of a distributed tensor.
#[derive(Debug, Clone)]
pub struct TensorShard {
    /// Which node holds this shard.
    pub node: NodeId,
    /// Shard index.
    pub index: usize,
    /// Shape of this shard (rows, cols).
    pub shape: (usize, usize),
    /// Flattened data.
    pub data: Vec<f64>,
}

/// A distributed tensor split across multiple nodes.
#[derive(Debug, Clone)]
pub struct DistributedTensor {
    /// Original shape (rows, cols).
    pub shape: (usize, usize),
    /// Dimension along which the tensor is sharded (0 = rows, 1 = cols).
    pub shard_dim: usize,
    /// Individual shards.
    pub shards: Vec<TensorShard>,
}

/// Shards a tensor along a dimension.
pub fn shard_tensor(
    data: &[f64],
    shape: (usize, usize),
    dim: usize,
    num_shards: usize,
    nodes: &[NodeId],
) -> DistributedTensor {
    let mut shards = Vec::new();

    if dim == 0 {
        // Shard along rows
        let rows_per_shard = shape.0 / num_shards;
        let remainder = shape.0 % num_shards;

        let mut row_offset = 0;
        for i in 0..num_shards {
            let extra = if i < remainder { 1 } else { 0 };
            let shard_rows = rows_per_shard + extra;
            let shard_shape = (shard_rows, shape.1);

            let start = row_offset * shape.1;
            let end = (row_offset + shard_rows) * shape.1;
            let shard_data = data[start..end].to_vec();

            shards.push(TensorShard {
                node: nodes[i % nodes.len()],
                index: i,
                shape: shard_shape,
                data: shard_data,
            });

            row_offset += shard_rows;
        }
    } else {
        // Shard along columns
        let cols_per_shard = shape.1 / num_shards;

        for i in 0..num_shards {
            let shard_shape = (shape.0, cols_per_shard);
            let mut shard_data = Vec::with_capacity(shape.0 * cols_per_shard);

            for row in 0..shape.0 {
                let start = row * shape.1 + i * cols_per_shard;
                let end = start + cols_per_shard;
                shard_data.extend_from_slice(&data[start..end]);
            }

            shards.push(TensorShard {
                node: nodes[i % nodes.len()],
                index: i,
                shape: shard_shape,
                data: shard_data,
            });
        }
    }

    DistributedTensor {
        shape,
        shard_dim: dim,
        shards,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.2: Shard Placement
// ═══════════════════════════════════════════════════════════════════════

/// Strategy for placing shards on nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementStrategy {
    /// Assign shards to nodes in round-robin order.
    RoundRobin,
    /// Place shards on nodes with most available memory.
    MemoryBalanced,
    /// Place shards on nodes closest to data source.
    LocalityAware,
}

impl fmt::Display for PlacementStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementStrategy::RoundRobin => write!(f, "RoundRobin"),
            PlacementStrategy::MemoryBalanced => write!(f, "MemoryBalanced"),
            PlacementStrategy::LocalityAware => write!(f, "LocalityAware"),
        }
    }
}

/// Assigns shards to nodes based on strategy.
pub fn place_shards(
    num_shards: usize,
    nodes: &[NodeId],
    strategy: &PlacementStrategy,
) -> Vec<NodeId> {
    match strategy {
        PlacementStrategy::RoundRobin => (0..num_shards).map(|i| nodes[i % nodes.len()]).collect(),
        PlacementStrategy::MemoryBalanced => {
            // In simulation, reverse round-robin (prefer later nodes with more memory)
            (0..num_shards)
                .map(|i| nodes[(nodes.len() - 1 - i % nodes.len()) % nodes.len()])
                .collect()
        }
        PlacementStrategy::LocalityAware => {
            // In simulation, prefer first node (closest)
            vec![nodes[0]; num_shards]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.3: Distributed MatMul
// ═══════════════════════════════════════════════════════════════════════

/// Performs local matrix multiplication on a shard.
pub fn local_matmul(
    a: &[f64],
    a_shape: (usize, usize),
    b: &[f64],
    b_shape: (usize, usize),
) -> Vec<f64> {
    let (m, k) = a_shape;
    let n = b_shape.1;
    let mut result = vec![0.0; m * n];

    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0;
            for p in 0..k {
                sum += a[i * k + p] * b[p * n + j];
            }
            result[i * n + j] = sum;
        }
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S10.4-S10.5: AllReduce & Ring AllReduce
// ═══════════════════════════════════════════════════════════════════════

/// Reduction operation for allreduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOp {
    /// Element-wise sum.
    Sum,
    /// Element-wise average.
    Average,
    /// Element-wise maximum.
    Max,
    /// Element-wise minimum.
    Min,
}

impl fmt::Display for ReduceOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReduceOp::Sum => write!(f, "Sum"),
            ReduceOp::Average => write!(f, "Average"),
            ReduceOp::Max => write!(f, "Max"),
            ReduceOp::Min => write!(f, "Min"),
        }
    }
}

/// Performs allreduce across multiple gradient vectors.
pub fn allreduce(gradients: &[Vec<f64>], op: ReduceOp) -> Vec<f64> {
    if gradients.is_empty() {
        return Vec::new();
    }

    let len = gradients[0].len();
    let mut result = vec![0.0; len];

    match op {
        ReduceOp::Sum => {
            for grad in gradients {
                for (i, &v) in grad.iter().enumerate() {
                    result[i] += v;
                }
            }
        }
        ReduceOp::Average => {
            for grad in gradients {
                for (i, &v) in grad.iter().enumerate() {
                    result[i] += v;
                }
            }
            let n = gradients.len() as f64;
            for v in &mut result {
                *v /= n;
            }
        }
        ReduceOp::Max => {
            result = gradients[0].clone();
            for grad in &gradients[1..] {
                for (i, &v) in grad.iter().enumerate() {
                    if v > result[i] {
                        result[i] = v;
                    }
                }
            }
        }
        ReduceOp::Min => {
            result = gradients[0].clone();
            for grad in &gradients[1..] {
                for (i, &v) in grad.iter().enumerate() {
                    if v < result[i] {
                        result[i] = v;
                    }
                }
            }
        }
    }

    result
}

/// Simulates ring allreduce topology: returns the number of communication steps.
pub fn ring_allreduce_steps(num_nodes: usize) -> usize {
    // Ring allreduce requires 2*(N-1) steps for N nodes
    if num_nodes <= 1 {
        0
    } else {
        2 * (num_nodes - 1)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.6: Parameter Server
// ═══════════════════════════════════════════════════════════════════════

/// A centralized parameter server.
#[derive(Debug)]
pub struct ParameterServer {
    /// Parameter values.
    pub params: Vec<f64>,
    /// Learning rate.
    pub learning_rate: f64,
    /// Number of gradient updates applied.
    pub update_count: u64,
}

impl ParameterServer {
    /// Creates a new parameter server with initial parameters.
    pub fn new(params: Vec<f64>, lr: f64) -> Self {
        ParameterServer {
            params,
            learning_rate: lr,
            update_count: 0,
        }
    }

    /// Applies a gradient update (SGD).
    pub fn apply_gradient(&mut self, gradients: &[f64]) {
        for (p, g) in self.params.iter_mut().zip(gradients.iter()) {
            *p -= self.learning_rate * g;
        }
        self.update_count += 1;
    }

    /// Returns a snapshot of current parameters.
    pub fn get_params(&self) -> &[f64] {
        &self.params
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.7-S10.8: Data Parallelism & Model Parallelism
// ═══════════════════════════════════════════════════════════════════════

/// Data parallelism: split a batch across nodes.
#[derive(Debug, Clone)]
pub struct DataParallelConfig {
    /// Number of worker nodes.
    pub num_workers: usize,
    /// Total batch size.
    pub batch_size: usize,
    /// Whether to synchronize gradients after each step.
    pub sync_gradients: bool,
}

impl DataParallelConfig {
    /// Returns the per-worker batch size.
    pub fn local_batch_size(&self) -> usize {
        self.batch_size / self.num_workers
    }
}

/// Model parallelism: split model layers across nodes.
#[derive(Debug, Clone)]
pub struct ModelParallelConfig {
    /// Number of pipeline stages.
    pub num_stages: usize,
    /// Layers per stage.
    pub layers_per_stage: Vec<Vec<String>>,
}

impl ModelParallelConfig {
    /// Creates a model parallel config by evenly dividing layers.
    pub fn even_split(layers: Vec<String>, num_stages: usize) -> Self {
        let per_stage = layers.len() / num_stages;
        let mut stages = Vec::new();
        for chunk in layers.chunks(per_stage) {
            stages.push(chunk.to_vec());
        }
        ModelParallelConfig {
            num_stages,
            layers_per_stage: stages,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S10.9: Communication Backend
// ═══════════════════════════════════════════════════════════════════════

/// Communication backend for collective operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommBackend {
    /// TCP sockets.
    Tcp,
    /// Shared memory (same machine).
    SharedMemory,
    /// NCCL (GPU-direct).
    Nccl,
}

impl fmt::Display for CommBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommBackend::Tcp => write!(f, "TCP"),
            CommBackend::SharedMemory => write!(f, "SharedMemory"),
            CommBackend::Nccl => write!(f, "NCCL"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S10.1 — Tensor Sharding
    #[test]
    fn s10_1_shard_rows() {
        let data: Vec<f64> = (0..12).map(|x| x as f64).collect();
        let nodes = vec![NodeId(0), NodeId(1)];
        let dt = shard_tensor(&data, (4, 3), 0, 2, &nodes);
        assert_eq!(dt.shards.len(), 2);
        assert_eq!(dt.shards[0].shape, (2, 3));
        assert_eq!(dt.shards[1].shape, (2, 3));
        assert_eq!(dt.shards[0].data, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn s10_1_shard_cols() {
        let data: Vec<f64> = (0..12).map(|x| x as f64).collect();
        let nodes = vec![NodeId(0), NodeId(1)];
        let dt = shard_tensor(&data, (3, 4), 1, 2, &nodes);
        assert_eq!(dt.shards.len(), 2);
        assert_eq!(dt.shards[0].shape, (3, 2));
    }

    // S10.2 — Shard Placement
    #[test]
    fn s10_2_round_robin_placement() {
        let nodes = vec![NodeId(0), NodeId(1), NodeId(2)];
        let placement = place_shards(5, &nodes, &PlacementStrategy::RoundRobin);
        assert_eq!(
            placement,
            vec![NodeId(0), NodeId(1), NodeId(2), NodeId(0), NodeId(1)]
        );
    }

    #[test]
    fn s10_2_locality_placement() {
        let nodes = vec![NodeId(0), NodeId(1)];
        let placement = place_shards(3, &nodes, &PlacementStrategy::LocalityAware);
        assert!(placement.iter().all(|&n| n == NodeId(0)));
    }

    // S10.3 — Distributed MatMul
    #[test]
    fn s10_3_local_matmul() {
        // 2x3 * 3x2 = 2x2
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let result = local_matmul(&a, (2, 3), &b, (3, 2));
        assert_eq!(result, vec![58.0, 64.0, 139.0, 154.0]);
    }

    // S10.4 — AllReduce Sum
    #[test]
    fn s10_4_allreduce_sum() {
        let grads = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let result = allreduce(&grads, ReduceOp::Sum);
        assert_eq!(result, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn s10_4_allreduce_average() {
        let grads = vec![vec![2.0, 4.0], vec![6.0, 8.0]];
        let result = allreduce(&grads, ReduceOp::Average);
        assert_eq!(result, vec![4.0, 6.0]);
    }

    // S10.5 — Ring AllReduce
    #[test]
    fn s10_5_ring_allreduce_steps() {
        assert_eq!(ring_allreduce_steps(1), 0);
        assert_eq!(ring_allreduce_steps(4), 6); // 2*(4-1) = 6
        assert_eq!(ring_allreduce_steps(8), 14);
    }

    // S10.6 — Parameter Server
    #[test]
    fn s10_6_param_server_update() {
        let mut ps = ParameterServer::new(vec![1.0, 2.0, 3.0], 0.1);
        ps.apply_gradient(&[10.0, 20.0, 30.0]);
        assert!((ps.params[0] - 0.0).abs() < 1e-10); // 1.0 - 0.1*10
        assert!((ps.params[1] - 0.0).abs() < 1e-10);
        assert!((ps.params[2] - 0.0).abs() < 1e-10);
        assert_eq!(ps.update_count, 1);
    }

    // S10.7 — Data Parallelism
    #[test]
    fn s10_7_data_parallel_batch_split() {
        let config = DataParallelConfig {
            num_workers: 4,
            batch_size: 64,
            sync_gradients: true,
        };
        assert_eq!(config.local_batch_size(), 16);
    }

    // S10.8 — Model Parallelism
    #[test]
    fn s10_8_model_parallel_split() {
        let layers = vec![
            "embed".into(),
            "attn1".into(),
            "ffn1".into(),
            "attn2".into(),
            "ffn2".into(),
            "head".into(),
        ];
        let config = ModelParallelConfig::even_split(layers, 3);
        assert_eq!(config.num_stages, 3);
        assert_eq!(config.layers_per_stage[0], vec!["embed", "attn1"]);
        assert_eq!(config.layers_per_stage[1], vec!["ffn1", "attn2"]);
    }

    // S10.9 — Communication Backend
    #[test]
    fn s10_9_comm_backend_display() {
        assert_eq!(CommBackend::Tcp.to_string(), "TCP");
        assert_eq!(CommBackend::SharedMemory.to_string(), "SharedMemory");
        assert_eq!(CommBackend::Nccl.to_string(), "NCCL");
    }

    // S10.10 — Integration
    #[test]
    fn s10_10_node_id_display() {
        assert_eq!(NodeId(42).to_string(), "Node(42)");
    }

    #[test]
    fn s10_10_reduce_op_display() {
        assert_eq!(ReduceOp::Sum.to_string(), "Sum");
        assert_eq!(ReduceOp::Max.to_string(), "Max");
    }

    #[test]
    fn s10_10_allreduce_max() {
        let grads = vec![vec![1.0, 5.0, 3.0], vec![4.0, 2.0, 6.0]];
        let result = allreduce(&grads, ReduceOp::Max);
        assert_eq!(result, vec![4.0, 5.0, 6.0]);
    }
}
