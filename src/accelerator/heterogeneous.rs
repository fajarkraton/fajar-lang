//! Heterogeneous execution — graph partitioning, data transfer, pipeline parallelism,
//! synchronization, memory pools, and multi-GPU split.

use std::collections::HashMap;
use std::fmt;

use super::dispatch::DispatchDevice;

// ═══════════════════════════════════════════════════════════════════════
// S35.1: Graph Partitioning
// ═══════════════════════════════════════════════════════════════════════

/// A node in the computation graph.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Unique node ID.
    pub id: u32,
    /// Operation name (e.g., "matmul", "relu", "conv2d").
    pub op: String,
    /// Input node IDs.
    pub inputs: Vec<u32>,
    /// Output size in bytes.
    pub output_bytes: u64,
    /// Estimated FLOPS for this operation.
    pub estimated_flops: u64,
    /// Assigned device (after partitioning).
    pub device: Option<DispatchDevice>,
}

/// Computation graph for heterogeneous execution.
#[derive(Debug, Clone)]
pub struct ComputeGraph {
    /// All nodes in topological order.
    pub nodes: Vec<GraphNode>,
}

impl ComputeGraph {
    /// Creates a new empty computation graph.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Adds a node to the graph.
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }

    /// Returns the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns nodes that have no inputs (roots).
    pub fn root_nodes(&self) -> Vec<&GraphNode> {
        self.nodes.iter().filter(|n| n.inputs.is_empty()).collect()
    }

    /// Returns nodes that are not referenced as input by any other node (outputs).
    pub fn output_nodes(&self) -> Vec<&GraphNode> {
        let referenced: std::collections::HashSet<u32> = self
            .nodes
            .iter()
            .flat_map(|n| n.inputs.iter().copied())
            .collect();
        self.nodes
            .iter()
            .filter(|n| !referenced.contains(&n.id))
            .collect()
    }

    /// Gets a node by ID.
    pub fn get_node(&self, id: u32) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

impl Default for ComputeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A subgraph assigned to a single device.
#[derive(Debug, Clone)]
pub struct Subgraph {
    /// Device this subgraph runs on.
    pub device: DispatchDevice,
    /// Node IDs in this subgraph.
    pub node_ids: Vec<u32>,
    /// Total estimated FLOPS.
    pub total_flops: u64,
    /// Total output bytes.
    pub total_output_bytes: u64,
}

/// Partitions a computation graph into per-device subgraphs.
///
/// Uses a greedy approach: assigns compute-heavy ops to GPU, small ops to CPU,
/// and INT8-compatible ops to NPU when available.
pub fn partition_graph(
    graph: &ComputeGraph,
    gpu_available: bool,
    npu_available: bool,
) -> Vec<Subgraph> {
    let mut cpu_nodes = Vec::new();
    let mut gpu_nodes = Vec::new();
    let mut npu_nodes = Vec::new();

    let mut cpu_flops = 0u64;
    let mut gpu_flops = 0u64;
    let mut npu_flops = 0u64;
    let mut cpu_bytes = 0u64;
    let mut gpu_bytes = 0u64;
    let mut npu_bytes = 0u64;

    for node in &graph.nodes {
        let is_heavy = node.estimated_flops > 100_000;
        let is_quantizable = matches!(node.op.as_str(), "matmul" | "conv2d" | "dense");

        if npu_available && is_quantizable && node.estimated_flops < 10_000_000 {
            npu_nodes.push(node.id);
            npu_flops += node.estimated_flops;
            npu_bytes += node.output_bytes;
        } else if gpu_available && is_heavy {
            gpu_nodes.push(node.id);
            gpu_flops += node.estimated_flops;
            gpu_bytes += node.output_bytes;
        } else {
            cpu_nodes.push(node.id);
            cpu_flops += node.estimated_flops;
            cpu_bytes += node.output_bytes;
        }
    }

    let mut subgraphs = Vec::new();

    if !cpu_nodes.is_empty() {
        subgraphs.push(Subgraph {
            device: DispatchDevice::Cpu,
            node_ids: cpu_nodes,
            total_flops: cpu_flops,
            total_output_bytes: cpu_bytes,
        });
    }

    if !gpu_nodes.is_empty() {
        subgraphs.push(Subgraph {
            device: DispatchDevice::Gpu(0),
            node_ids: gpu_nodes,
            total_flops: gpu_flops,
            total_output_bytes: gpu_bytes,
        });
    }

    if !npu_nodes.is_empty() {
        subgraphs.push(Subgraph {
            device: DispatchDevice::Npu(0),
            node_ids: npu_nodes,
            total_flops: npu_flops,
            total_output_bytes: npu_bytes,
        });
    }

    subgraphs
}

// ═══════════════════════════════════════════════════════════════════════
// S35.2: Data Transfer
// ═══════════════════════════════════════════════════════════════════════

/// Direction of a data transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// Host (CPU) to device (GPU/NPU).
    HostToDevice,
    /// Device (GPU/NPU) to host (CPU).
    DeviceToHost,
    /// Device to device (peer transfer).
    DeviceToDevice,
}

impl fmt::Display for TransferDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostToDevice => write!(f, "H2D"),
            Self::DeviceToHost => write!(f, "D2H"),
            Self::DeviceToDevice => write!(f, "D2D"),
        }
    }
}

/// A data transfer operation between devices.
#[derive(Debug, Clone)]
pub struct DataTransfer {
    /// Source device.
    pub src: DispatchDevice,
    /// Destination device.
    pub dst: DispatchDevice,
    /// Transfer direction.
    pub direction: TransferDirection,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Whether to use pinned (page-locked) memory.
    pub pinned: bool,
    /// Estimated transfer time in microseconds.
    pub estimated_us: u64,
}

impl DataTransfer {
    /// Creates a host-to-device transfer.
    pub fn host_to_device(dst: DispatchDevice, size_bytes: u64, pinned: bool) -> Self {
        // Estimate: PCIe 4.0 ~25 GB/s, pinned ~2x
        let bw_gbps = if pinned { 25.0 } else { 12.0 };
        let estimated_us = (size_bytes as f64 / (bw_gbps * 1e3)) as u64;

        Self {
            src: DispatchDevice::Cpu,
            dst,
            direction: TransferDirection::HostToDevice,
            size_bytes,
            pinned,
            estimated_us,
        }
    }

    /// Creates a device-to-host transfer.
    pub fn device_to_host(src: DispatchDevice, size_bytes: u64, pinned: bool) -> Self {
        let bw_gbps = if pinned { 25.0 } else { 12.0 };
        let estimated_us = (size_bytes as f64 / (bw_gbps * 1e3)) as u64;

        Self {
            src,
            dst: DispatchDevice::Cpu,
            direction: TransferDirection::DeviceToHost,
            size_bytes,
            pinned,
            estimated_us,
        }
    }

    /// Creates a device-to-device transfer.
    pub fn device_to_device(src: DispatchDevice, dst: DispatchDevice, size_bytes: u64) -> Self {
        // D2D uses NVLink if available, otherwise PCIe
        let bw_gbps = 50.0; // NVLink estimate
        let estimated_us = (size_bytes as f64 / (bw_gbps * 1e3)) as u64;

        Self {
            src,
            dst,
            direction: TransferDirection::DeviceToDevice,
            size_bytes,
            pinned: false,
            estimated_us,
        }
    }
}

/// Computes required data transfers between subgraphs.
pub fn compute_transfers(graph: &ComputeGraph, subgraphs: &[Subgraph]) -> Vec<DataTransfer> {
    let mut transfers = Vec::new();

    // Build node->device map
    let mut node_device: HashMap<u32, DispatchDevice> = HashMap::new();
    for sg in subgraphs {
        for &nid in &sg.node_ids {
            node_device.insert(nid, sg.device);
        }
    }

    // For each node, check if its inputs are on a different device
    for node in &graph.nodes {
        let my_device = match node_device.get(&node.id) {
            Some(d) => *d,
            None => continue,
        };

        for &input_id in &node.inputs {
            let input_device = match node_device.get(&input_id) {
                Some(d) => *d,
                None => continue,
            };

            if input_device != my_device {
                let input_node = graph.get_node(input_id);
                let size = input_node.map(|n| n.output_bytes).unwrap_or(0);

                let transfer = match (input_device, my_device) {
                    (DispatchDevice::Cpu, _) => DataTransfer::host_to_device(my_device, size, true),
                    (_, DispatchDevice::Cpu) => {
                        DataTransfer::device_to_host(input_device, size, true)
                    }
                    _ => DataTransfer::device_to_device(input_device, my_device, size),
                };
                transfers.push(transfer);
            }
        }
    }

    transfers
}

// ═══════════════════════════════════════════════════════════════════════
// S35.3: Transfer Optimization (Double Buffering)
// ═══════════════════════════════════════════════════════════════════════

/// Double buffer state for overlapping compute and transfer.
#[derive(Debug, Clone)]
pub struct DoubleBuffer {
    /// Buffer A size in bytes.
    pub buffer_a_bytes: u64,
    /// Buffer B size in bytes.
    pub buffer_b_bytes: u64,
    /// Which buffer is currently active (0 = A, 1 = B).
    pub active: u8,
}

impl DoubleBuffer {
    /// Creates a new double buffer with the given size per buffer.
    pub fn new(buffer_size: u64) -> Self {
        Self {
            buffer_a_bytes: buffer_size,
            buffer_b_bytes: buffer_size,
            active: 0,
        }
    }

    /// Swaps the active buffer.
    pub fn swap(&mut self) {
        self.active = 1 - self.active;
    }

    /// Returns the total memory used (both buffers).
    pub fn total_bytes(&self) -> u64 {
        self.buffer_a_bytes + self.buffer_b_bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S35.4: Pipeline Parallelism
// ═══════════════════════════════════════════════════════════════════════

/// A stage in a pipeline parallel execution.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Stage index.
    pub index: u32,
    /// Device for this stage.
    pub device: DispatchDevice,
    /// Subgraph node IDs for this stage.
    pub node_ids: Vec<u32>,
    /// Estimated execution time in microseconds.
    pub estimated_us: u64,
}

/// Pipeline parallel execution plan.
#[derive(Debug, Clone)]
pub struct PipelinePlan {
    /// Ordered pipeline stages.
    pub stages: Vec<PipelineStage>,
    /// Number of microbatches.
    pub microbatches: u32,
}

impl PipelinePlan {
    /// Creates a pipeline plan from subgraphs.
    pub fn from_subgraphs(subgraphs: &[Subgraph], microbatches: u32) -> Self {
        let stages: Vec<PipelineStage> = subgraphs
            .iter()
            .enumerate()
            .map(|(i, sg)| PipelineStage {
                index: i as u32,
                device: sg.device,
                node_ids: sg.node_ids.clone(),
                estimated_us: estimate_stage_time(sg),
            })
            .collect();

        Self {
            stages,
            microbatches,
        }
    }

    /// Returns the estimated total pipeline latency.
    ///
    /// With pipeline parallelism: fill_time + (microbatches - 1) * stage_time
    pub fn estimated_total_us(&self) -> u64 {
        if self.stages.is_empty() {
            return 0;
        }

        let max_stage = self
            .stages
            .iter()
            .map(|s| s.estimated_us)
            .max()
            .unwrap_or(0);
        let fill_time: u64 = self.stages.iter().map(|s| s.estimated_us).sum();
        let steady_state = if self.microbatches > 1 {
            (self.microbatches as u64 - 1) * max_stage
        } else {
            0
        };

        fill_time + steady_state
    }

    /// Returns the pipeline efficiency (ideal speedup / actual).
    pub fn efficiency(&self) -> f64 {
        if self.stages.is_empty() {
            return 0.0;
        }

        let sequential: u64 = self
            .stages
            .iter()
            .map(|s| s.estimated_us * self.microbatches as u64)
            .sum();
        let pipeline = self.estimated_total_us();

        if pipeline == 0 {
            0.0
        } else {
            sequential as f64 / pipeline as f64
        }
    }

    /// Returns the number of stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

fn estimate_stage_time(sg: &Subgraph) -> u64 {
    // Rough estimate: 1 GFLOPS = 1ms
    (sg.total_flops / 1_000_000).max(1)
}

// ═══════════════════════════════════════════════════════════════════════
// S35.5: Synchronization
// ═══════════════════════════════════════════════════════════════════════

/// Synchronization barrier between pipeline stages.
#[derive(Debug, Clone)]
pub struct Barrier {
    /// Barrier ID.
    pub id: u32,
    /// Devices that must synchronize.
    pub devices: Vec<DispatchDevice>,
    /// Whether the barrier has been reached by all devices.
    pub completed: bool,
}

impl Barrier {
    /// Creates a new barrier.
    pub fn new(id: u32, devices: Vec<DispatchDevice>) -> Self {
        Self {
            id,
            devices,
            completed: false,
        }
    }

    /// Marks the barrier as completed.
    pub fn complete(&mut self) {
        self.completed = true;
    }

    /// Returns the number of devices in the barrier.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

/// Computes synchronization barriers needed between subgraph stages.
pub fn compute_barriers(subgraphs: &[Subgraph]) -> Vec<Barrier> {
    let mut barriers = Vec::new();

    for (i, window) in subgraphs.windows(2).enumerate() {
        if window[0].device != window[1].device {
            barriers.push(Barrier::new(
                i as u32,
                vec![window[0].device, window[1].device],
            ));
        }
    }

    barriers
}

// ═══════════════════════════════════════════════════════════════════════
// S35.6: Memory Pool
// ═══════════════════════════════════════════════════════════════════════

/// Pre-allocated device memory pool.
#[derive(Debug, Clone)]
pub struct DeviceMemoryPool {
    /// Device this pool belongs to.
    pub device: DispatchDevice,
    /// Total pool size in bytes.
    pub total_bytes: u64,
    /// Currently allocated bytes.
    pub used_bytes: u64,
    /// Number of active allocations.
    pub allocation_count: u32,
}

impl DeviceMemoryPool {
    /// Creates a new memory pool.
    pub fn new(device: DispatchDevice, total_bytes: u64) -> Self {
        Self {
            device,
            total_bytes,
            used_bytes: 0,
            allocation_count: 0,
        }
    }

    /// Allocates from the pool.
    pub fn allocate(&mut self, bytes: u64) -> Result<u64, String> {
        if self.used_bytes + bytes > self.total_bytes {
            return Err(format!(
                "pool exhausted: need {bytes}B but only {}B free on {}",
                self.total_bytes - self.used_bytes,
                self.device
            ));
        }
        let offset = self.used_bytes;
        self.used_bytes += bytes;
        self.allocation_count += 1;
        Ok(offset)
    }

    /// Frees memory back to the pool.
    pub fn free(&mut self, bytes: u64) {
        self.used_bytes = self.used_bytes.saturating_sub(bytes);
        self.allocation_count = self.allocation_count.saturating_sub(1);
    }

    /// Returns the fraction of the pool in use.
    pub fn utilization(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes as f64 / self.total_bytes as f64
        }
    }

    /// Returns remaining free bytes.
    pub fn free_bytes(&self) -> u64 {
        self.total_bytes - self.used_bytes
    }

    /// Resets the pool (frees all allocations).
    pub fn reset(&mut self) {
        self.used_bytes = 0;
        self.allocation_count = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S35.7: Multi-GPU Split
// ═══════════════════════════════════════════════════════════════════════

/// Strategy for splitting across multiple GPUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiGpuStrategy {
    /// Split layers across GPUs (pipeline parallel).
    PipelineParallel,
    /// Split tensors across GPUs (tensor parallel).
    TensorParallel,
    /// Replicate model on each GPU with data parallel.
    DataParallel,
}

impl fmt::Display for MultiGpuStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PipelineParallel => write!(f, "pipeline-parallel"),
            Self::TensorParallel => write!(f, "tensor-parallel"),
            Self::DataParallel => write!(f, "data-parallel"),
        }
    }
}

/// Multi-GPU split configuration.
#[derive(Debug, Clone)]
pub struct MultiGpuPlan {
    /// Strategy used.
    pub strategy: MultiGpuStrategy,
    /// GPU indices involved.
    pub gpu_ids: Vec<u32>,
    /// How layers/tensors are distributed.
    pub assignments: Vec<(u32, Vec<u32>)>, // (gpu_id, node_ids)
}

impl MultiGpuPlan {
    /// Creates a pipeline-parallel split across GPUs.
    pub fn pipeline_split(graph: &ComputeGraph, gpu_ids: &[u32]) -> Self {
        let nodes_per_gpu = graph.node_count() / gpu_ids.len().max(1);
        let mut assignments = Vec::new();

        for (i, &gpu_id) in gpu_ids.iter().enumerate() {
            let start = i * nodes_per_gpu;
            let end = if i == gpu_ids.len() - 1 {
                graph.node_count()
            } else {
                start + nodes_per_gpu
            };
            let node_ids: Vec<u32> = graph.nodes[start..end].iter().map(|n| n.id).collect();
            assignments.push((gpu_id, node_ids));
        }

        Self {
            strategy: MultiGpuStrategy::PipelineParallel,
            gpu_ids: gpu_ids.to_vec(),
            assignments,
        }
    }

    /// Returns the number of GPUs in the plan.
    pub fn gpu_count(&self) -> usize {
        self.gpu_ids.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S35.8: Heterogeneous Batch
// ═══════════════════════════════════════════════════════════════════════

/// A batch item assigned to a specific device.
#[derive(Debug, Clone)]
pub struct BatchAssignment {
    /// Batch index.
    pub batch_idx: u32,
    /// Assigned device.
    pub device: DispatchDevice,
}

/// Assigns batch items to different accelerators.
pub fn assign_heterogeneous_batch(
    batch_size: u32,
    devices: &[DispatchDevice],
) -> Vec<BatchAssignment> {
    if devices.is_empty() {
        return Vec::new();
    }

    let mut assignments = Vec::new();
    for i in 0..batch_size {
        let device_idx = (i as usize) % devices.len();
        assignments.push(BatchAssignment {
            batch_idx: i,
            device: devices[device_idx],
        });
    }
    assignments
}

// ═══════════════════════════════════════════════════════════════════════
// S35.9: Execution Plan Visualization
// ═══════════════════════════════════════════════════════════════════════

/// Generates a DOT graph visualization of the execution plan.
pub fn generate_dot_graph(graph: &ComputeGraph, subgraphs: &[Subgraph]) -> String {
    let mut dot = String::new();
    dot.push_str("digraph execution_plan {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box, style=filled];\n\n");

    // Build node->device map
    let mut node_device: HashMap<u32, DispatchDevice> = HashMap::new();
    for sg in subgraphs {
        for &nid in &sg.node_ids {
            node_device.insert(nid, sg.device);
        }
    }

    // Color by device
    for node in &graph.nodes {
        let color = match node_device.get(&node.id) {
            Some(DispatchDevice::Cpu) => "#a0c4ff",
            Some(DispatchDevice::Gpu(_)) => "#ffc6ff",
            Some(DispatchDevice::Npu(_)) => "#caffbf",
            None => "#ffffff",
        };
        let device_label = node_device
            .get(&node.id)
            .map(|d| format!("{d}"))
            .unwrap_or_else(|| "?".to_string());
        dot.push_str(&format!(
            "  n{} [label=\"{}\\n({})\\n{}\" fillcolor=\"{}\"];\n",
            node.id, node.op, node.id, device_label, color
        ));
    }

    dot.push('\n');

    // Edges
    for node in &graph.nodes {
        for &input_id in &node.inputs {
            let src_device = node_device.get(&input_id);
            let dst_device = node_device.get(&node.id);
            let style = if src_device != dst_device {
                " [style=dashed, color=red, label=\"transfer\"]"
            } else {
                ""
            };
            dot.push_str(&format!("  n{input_id} -> n{}{style};\n", node.id));
        }
    }

    dot.push_str("}\n");
    dot
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_graph() -> ComputeGraph {
        let mut g = ComputeGraph::new();
        g.add_node(GraphNode {
            id: 0,
            op: "input".to_string(),
            inputs: vec![],
            output_bytes: 4096,
            estimated_flops: 0,
            device: None,
        });
        g.add_node(GraphNode {
            id: 1,
            op: "matmul".to_string(),
            inputs: vec![0],
            output_bytes: 4096,
            estimated_flops: 1_000_000,
            device: None,
        });
        g.add_node(GraphNode {
            id: 2,
            op: "relu".to_string(),
            inputs: vec![1],
            output_bytes: 4096,
            estimated_flops: 1000,
            device: None,
        });
        g.add_node(GraphNode {
            id: 3,
            op: "matmul".to_string(),
            inputs: vec![2],
            output_bytes: 1024,
            estimated_flops: 500_000,
            device: None,
        });
        g.add_node(GraphNode {
            id: 4,
            op: "softmax".to_string(),
            inputs: vec![3],
            output_bytes: 1024,
            estimated_flops: 5000,
            device: None,
        });
        g
    }

    // S35.1: Graph partitioning
    #[test]
    fn s35_1_partition_cpu_only() {
        let g = test_graph();
        let sgs = partition_graph(&g, false, false);
        assert_eq!(sgs.len(), 1);
        assert_eq!(sgs[0].device, DispatchDevice::Cpu);
        assert_eq!(sgs[0].node_ids.len(), 5);
    }

    #[test]
    fn s35_1_partition_with_gpu() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, false);
        assert!(sgs.len() >= 2);
        let gpu_sg = sgs
            .iter()
            .find(|s| matches!(s.device, DispatchDevice::Gpu(_)));
        assert!(gpu_sg.is_some());
    }

    #[test]
    fn s35_1_graph_root_and_output() {
        let g = test_graph();
        let roots = g.root_nodes();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].id, 0);

        let outputs = g.output_nodes();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].id, 4);
    }

    // S35.2: Data transfer
    #[test]
    fn s35_2_host_to_device_transfer() {
        let t = DataTransfer::host_to_device(DispatchDevice::Gpu(0), 1_000_000, true);
        assert_eq!(t.direction, TransferDirection::HostToDevice);
        assert_eq!(t.size_bytes, 1_000_000);
        assert!(t.pinned);
    }

    #[test]
    fn s35_2_compute_transfers() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, false);
        let transfers = compute_transfers(&g, &sgs);
        // Transfers needed at GPU/CPU boundaries
        if sgs.len() > 1 {
            assert!(!transfers.is_empty());
        }
    }

    // S35.3: Double buffering
    #[test]
    fn s35_3_double_buffer() {
        let mut db = DoubleBuffer::new(1024);
        assert_eq!(db.active, 0);
        assert_eq!(db.total_bytes(), 2048);
        db.swap();
        assert_eq!(db.active, 1);
        db.swap();
        assert_eq!(db.active, 0);
    }

    // S35.4: Pipeline parallelism
    #[test]
    fn s35_4_pipeline_plan() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, false);
        let plan = PipelinePlan::from_subgraphs(&sgs, 4);
        assert!(plan.stage_count() >= 1);
        assert!(plan.estimated_total_us() > 0);
        assert!(plan.efficiency() > 0.0);
    }

    #[test]
    fn s35_4_pipeline_empty() {
        let plan = PipelinePlan::from_subgraphs(&[], 1);
        assert_eq!(plan.estimated_total_us(), 0);
        assert_eq!(plan.efficiency(), 0.0);
    }

    // S35.5: Synchronization
    #[test]
    fn s35_5_barriers() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, false);
        let barriers = compute_barriers(&sgs);
        // Barriers needed at device transitions
        for b in &barriers {
            assert_eq!(b.device_count(), 2);
            assert!(!b.completed);
        }
    }

    #[test]
    fn s35_5_barrier_complete() {
        let mut b = Barrier::new(0, vec![DispatchDevice::Cpu, DispatchDevice::Gpu(0)]);
        assert!(!b.completed);
        b.complete();
        assert!(b.completed);
    }

    // S35.6: Memory pool
    #[test]
    fn s35_6_memory_pool_allocate() {
        let mut pool = DeviceMemoryPool::new(DispatchDevice::Gpu(0), 1024);
        assert_eq!(pool.free_bytes(), 1024);

        let offset = pool.allocate(256).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(pool.free_bytes(), 768);
        assert_eq!(pool.allocation_count, 1);
    }

    #[test]
    fn s35_6_memory_pool_exhausted() {
        let mut pool = DeviceMemoryPool::new(DispatchDevice::Gpu(0), 100);
        assert!(pool.allocate(50).is_ok());
        assert!(pool.allocate(60).is_err()); // Would exceed total
    }

    #[test]
    fn s35_6_memory_pool_utilization() {
        let mut pool = DeviceMemoryPool::new(DispatchDevice::Gpu(0), 1000);
        pool.allocate(500).unwrap();
        assert!((pool.utilization() - 0.5).abs() < 0.01);
    }

    // S35.7: Multi-GPU split
    #[test]
    fn s35_7_multi_gpu_pipeline_split() {
        let g = test_graph();
        let plan = MultiGpuPlan::pipeline_split(&g, &[0, 1]);
        assert_eq!(plan.gpu_count(), 2);
        assert_eq!(plan.strategy, MultiGpuStrategy::PipelineParallel);
        // All nodes should be assigned
        let total_nodes: usize = plan.assignments.iter().map(|(_, ns)| ns.len()).sum();
        assert_eq!(total_nodes, g.node_count());
    }

    // S35.8: Heterogeneous batch
    #[test]
    fn s35_8_heterogeneous_batch() {
        let devices = vec![DispatchDevice::Gpu(0), DispatchDevice::Npu(0)];
        let assignments = assign_heterogeneous_batch(4, &devices);
        assert_eq!(assignments.len(), 4);
        assert_eq!(assignments[0].device, DispatchDevice::Gpu(0));
        assert_eq!(assignments[1].device, DispatchDevice::Npu(0));
        assert_eq!(assignments[2].device, DispatchDevice::Gpu(0));
        assert_eq!(assignments[3].device, DispatchDevice::Npu(0));
    }

    #[test]
    fn s35_8_empty_devices() {
        let assignments = assign_heterogeneous_batch(4, &[]);
        assert!(assignments.is_empty());
    }

    // S35.9: DOT graph visualization
    #[test]
    fn s35_9_dot_graph() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, false);
        let dot = generate_dot_graph(&g, &sgs);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("matmul"));
        assert!(dot.contains("relu"));
    }

    // S35.10: Integration tests
    #[test]
    fn s35_10_full_pipeline() {
        let g = test_graph();
        let sgs = partition_graph(&g, true, true);
        let transfers = compute_transfers(&g, &sgs);
        let barriers = compute_barriers(&sgs);
        let plan = PipelinePlan::from_subgraphs(&sgs, 4);
        let dot = generate_dot_graph(&g, &sgs);

        assert!(!sgs.is_empty());
        assert!(plan.estimated_total_us() > 0);
        assert!(dot.contains("digraph"));
        // Ensure all components are generated
        let _ = (transfers, barriers);
    }

    #[test]
    fn s35_10_transfer_direction_display() {
        assert_eq!(format!("{}", TransferDirection::HostToDevice), "H2D");
        assert_eq!(format!("{}", TransferDirection::DeviceToHost), "D2H");
        assert_eq!(format!("{}", TransferDirection::DeviceToDevice), "D2D");
    }

    #[test]
    fn s35_10_multi_gpu_strategy_display() {
        assert_eq!(
            format!("{}", MultiGpuStrategy::PipelineParallel),
            "pipeline-parallel"
        );
        assert_eq!(
            format!("{}", MultiGpuStrategy::TensorParallel),
            "tensor-parallel"
        );
        assert_eq!(
            format!("{}", MultiGpuStrategy::DataParallel),
            "data-parallel"
        );
    }
}
