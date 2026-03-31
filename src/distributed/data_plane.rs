//! Data Plane — data partitioning, transfer protocol, serialization,
//! optional LZ4 compression, scatter, gather, broadcast,
//! ring-allreduce, pipeline parallelism.
//!
//! Sprint D4: Data Plane (10 tasks)
//! All simulated — no real networking.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D4.1: Data Partitioning
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a data partition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartitionId(pub u64);

impl fmt::Display for PartitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Part({})", self.0)
    }
}

/// Unique identifier for a data plane node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataNodeId(pub u64);

impl fmt::Display for DataNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DNode({})", self.0)
    }
}

/// Partitioning strategy for data shards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionStrategy {
    /// Shard by rows (horizontal partitioning).
    ByRows,
    /// Shard by columns (vertical partitioning).
    ByColumns,
    /// Hash-based partitioning on a key column.
    Hash { key_column: usize },
    /// Range-based partitioning.
    Range { column: usize },
}

impl fmt::Display for PartitionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartitionStrategy::ByRows => write!(f, "ByRows"),
            PartitionStrategy::ByColumns => write!(f, "ByColumns"),
            PartitionStrategy::Hash { key_column } => write!(f, "Hash(col={key_column})"),
            PartitionStrategy::Range { column } => write!(f, "Range(col={column})"),
        }
    }
}

/// A data partition (shard).
#[derive(Debug, Clone)]
pub struct DataPartition {
    /// Partition ID.
    pub id: PartitionId,
    /// Assigned node.
    pub node: DataNodeId,
    /// Shape (rows, cols).
    pub shape: (usize, usize),
    /// Flattened f64 data.
    pub data: Vec<f64>,
}

/// Partitions a 2D dataset into N shards by rows.
pub fn partition_by_rows(
    data: &[f64],
    shape: (usize, usize),
    num_partitions: usize,
    nodes: &[DataNodeId],
) -> Vec<DataPartition> {
    let rows = shape.0;
    let cols = shape.1;
    let rows_per_part = rows / num_partitions;
    let remainder = rows % num_partitions;
    let mut partitions = Vec::new();
    let mut row_offset = 0;

    for i in 0..num_partitions {
        let extra = if i < remainder { 1 } else { 0 };
        let part_rows = rows_per_part + extra;
        let start = row_offset * cols;
        let end = (row_offset + part_rows) * cols;
        let part_data = data[start..end].to_vec();

        partitions.push(DataPartition {
            id: PartitionId(i as u64),
            node: nodes[i % nodes.len()],
            shape: (part_rows, cols),
            data: part_data,
        });
        row_offset += part_rows;
    }
    partitions
}

/// Partitions a 2D dataset into N shards by columns.
pub fn partition_by_columns(
    data: &[f64],
    shape: (usize, usize),
    num_partitions: usize,
    nodes: &[DataNodeId],
) -> Vec<DataPartition> {
    let rows = shape.0;
    let cols = shape.1;
    let cols_per_part = cols / num_partitions;
    let mut partitions = Vec::new();

    for i in 0..num_partitions {
        let mut part_data = Vec::with_capacity(rows * cols_per_part);
        for row in 0..rows {
            let start = row * cols + i * cols_per_part;
            let end = start + cols_per_part;
            part_data.extend_from_slice(&data[start..end]);
        }
        partitions.push(DataPartition {
            id: PartitionId(i as u64),
            node: nodes[i % nodes.len()],
            shape: (rows, cols_per_part),
            data: part_data,
        });
    }
    partitions
}

// ═══════════════════════════════════════════════════════════════════════
// D4.2: Transfer Protocol
// ═══════════════════════════════════════════════════════════════════════

/// Transfer message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    /// Data push from sender to receiver.
    Push,
    /// Data pull request.
    Pull,
    /// Acknowledgment.
    Ack,
    /// Error/negative acknowledgment.
    Nack,
}

impl fmt::Display for TransferType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferType::Push => write!(f, "Push"),
            TransferType::Pull => write!(f, "Pull"),
            TransferType::Ack => write!(f, "Ack"),
            TransferType::Nack => write!(f, "Nack"),
        }
    }
}

/// A transfer message in the data plane.
#[derive(Debug, Clone)]
pub struct TransferMessage {
    /// Message type.
    pub msg_type: TransferType,
    /// Source node.
    pub source: DataNodeId,
    /// Destination node.
    pub destination: DataNodeId,
    /// Partition being transferred.
    pub partition_id: PartitionId,
    /// Sequence number for ordering.
    pub seq: u64,
    /// Payload bytes.
    pub payload: Vec<u8>,
}

/// Simulated transfer log.
#[derive(Debug, Default)]
pub struct TransferLog {
    /// All transfer messages.
    messages: Vec<TransferMessage>,
}

impl TransferLog {
    /// Creates a new transfer log.
    pub fn new() -> Self {
        TransferLog::default()
    }

    /// Records a transfer message.
    pub fn record(&mut self, msg: TransferMessage) {
        self.messages.push(msg);
    }

    /// Returns all messages for a given destination.
    pub fn messages_for(&self, dest: DataNodeId) -> Vec<&TransferMessage> {
        self.messages
            .iter()
            .filter(|m| m.destination == dest)
            .collect()
    }

    /// Returns the total number of recorded messages.
    pub fn total_messages(&self) -> usize {
        self.messages.len()
    }

    /// Total payload bytes transferred.
    pub fn total_bytes(&self) -> usize {
        self.messages.iter().map(|m| m.payload.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D4.3: Serialization (Header + Raw)
// ═══════════════════════════════════════════════════════════════════════

/// Data frame header.
#[derive(Debug, Clone, PartialEq)]
pub struct DataFrameHeader {
    /// Shape (rows, cols).
    pub shape: (usize, usize),
    /// Data type tag.
    pub dtype: DataType,
    /// Whether payload is compressed.
    pub compressed: bool,
    /// Payload length in bytes.
    pub payload_len: usize,
}

/// Supported data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 32-bit integer.
    I32,
    /// 64-bit integer.
    I64,
    /// Raw bytes.
    Bytes,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::F32 => write!(f, "f32"),
            DataType::F64 => write!(f, "f64"),
            DataType::I32 => write!(f, "i32"),
            DataType::I64 => write!(f, "i64"),
            DataType::Bytes => write!(f, "bytes"),
        }
    }
}

/// Serializes f64 data into a frame (header + raw bytes).
pub fn serialize_frame(data: &[f64], shape: (usize, usize), compressed: bool) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header: [rows:8][cols:8][dtype:1][compressed:1][payload_len:8]
    buf.extend_from_slice(&(shape.0 as u64).to_le_bytes());
    buf.extend_from_slice(&(shape.1 as u64).to_le_bytes());
    buf.push(1); // dtype = f64
    buf.push(if compressed { 1 } else { 0 });

    let raw: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();

    let payload = if compressed { lz4_compress(&raw) } else { raw };

    buf.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    buf.extend_from_slice(&payload);
    buf
}

/// Deserializes a frame back to header + f64 data.
pub fn deserialize_frame(buf: &[u8]) -> Option<(DataFrameHeader, Vec<f64>)> {
    if buf.len() < 26 {
        return None; // Header is 26 bytes minimum
    }

    let rows = u64::from_le_bytes(buf[0..8].try_into().ok()?) as usize;
    let cols = u64::from_le_bytes(buf[8..16].try_into().ok()?) as usize;
    let dtype = match buf[16] {
        0 => DataType::F32,
        1 => DataType::F64,
        2 => DataType::I32,
        3 => DataType::I64,
        _ => DataType::Bytes,
    };
    let compressed = buf[17] != 0;
    let payload_len = u64::from_le_bytes(buf[18..26].try_into().ok()?) as usize;

    if buf.len() < 26 + payload_len {
        return None;
    }

    let payload = &buf[26..26 + payload_len];
    let raw = if compressed {
        lz4_decompress(payload)
    } else {
        payload.to_vec()
    };

    let data: Vec<f64> = raw
        .chunks_exact(8)
        .filter_map(|chunk| chunk.try_into().ok().map(f64::from_le_bytes))
        .collect();

    let header = DataFrameHeader {
        shape: (rows, cols),
        dtype,
        compressed,
        payload_len,
    };

    Some((header, data))
}

// ═══════════════════════════════════════════════════════════════════════
// D4.4: Optional LZ4 Compression (simulated)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated LZ4 compression (simple run-length encoding for demonstration).
pub fn lz4_compress(data: &[u8]) -> Vec<u8> {
    // Simple RLE-like compression: [literal_count][literals][repeat_count][byte]
    // For simulation, we just store length + data (pass-through with length prefix).
    let mut out = Vec::with_capacity(4 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// Simulated LZ4 decompression.
pub fn lz4_decompress(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return Vec::new();
    }
    let len = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0; 4])) as usize;
    if data.len() < 4 + len {
        return Vec::new();
    }
    data[4..4 + len].to_vec()
}

/// Returns the compression ratio (compressed / original).
pub fn compression_ratio(original_len: usize, compressed_len: usize) -> f64 {
    if original_len == 0 {
        return 1.0;
    }
    compressed_len as f64 / original_len as f64
}

// ═══════════════════════════════════════════════════════════════════════
// D4.5: Scatter
// ═══════════════════════════════════════════════════════════════════════

/// Scatter operation: distributes data from one node to all nodes.
pub fn scatter(
    data: &[f64],
    shape: (usize, usize),
    _source: DataNodeId,
    targets: &[DataNodeId],
) -> Vec<DataPartition> {
    partition_by_rows(data, shape, targets.len(), targets)
        .into_iter()
        .enumerate()
        .map(|(i, mut part)| {
            part.id = PartitionId(i as u64);
            part.node = targets[i];
            part
        })
        .collect()
}

/// Returns the sizes of scattered partitions.
pub fn scatter_sizes(total_rows: usize, num_targets: usize) -> Vec<usize> {
    let base = total_rows / num_targets;
    let rem = total_rows % num_targets;
    (0..num_targets)
        .map(|i| base + if i < rem { 1 } else { 0 })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// D4.6: Gather
// ═══════════════════════════════════════════════════════════════════════

/// Gather operation: collects partitions from all nodes into one dataset.
pub fn gather(partitions: &[DataPartition], total_shape: (usize, usize)) -> Vec<f64> {
    let mut result = Vec::with_capacity(total_shape.0 * total_shape.1);
    // Sort by partition ID to maintain order.
    let mut sorted: Vec<&DataPartition> = partitions.iter().collect();
    sorted.sort_by_key(|p| p.id.0);
    for part in sorted {
        result.extend_from_slice(&part.data);
    }
    result
}

/// Validates that gathered data matches the expected shape.
pub fn validate_gather(partitions: &[DataPartition], expected_shape: (usize, usize)) -> bool {
    let total_elements: usize = partitions.iter().map(|p| p.data.len()).sum();
    total_elements == expected_shape.0 * expected_shape.1
}

// ═══════════════════════════════════════════════════════════════════════
// D4.7: Broadcast
// ═══════════════════════════════════════════════════════════════════════

/// Broadcast operation: replicates data from one node to all nodes.
pub fn broadcast(
    data: &[f64],
    shape: (usize, usize),
    _source: DataNodeId,
    targets: &[DataNodeId],
) -> Vec<DataPartition> {
    targets
        .iter()
        .enumerate()
        .map(|(i, &node)| DataPartition {
            id: PartitionId(i as u64),
            node,
            shape,
            data: data.to_vec(),
        })
        .collect()
}

/// Returns the total bytes transmitted in a broadcast.
pub fn broadcast_bandwidth(data_bytes: usize, num_targets: usize) -> usize {
    data_bytes * num_targets
}

// ═══════════════════════════════════════════════════════════════════════
// D4.8: Ring-AllReduce
// ═══════════════════════════════════════════════════════════════════════

/// Ring allreduce operation for gradient aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOperation {
    /// Element-wise sum.
    Sum,
    /// Element-wise average.
    Average,
    /// Element-wise max.
    Max,
}

impl fmt::Display for ReduceOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReduceOperation::Sum => write!(f, "Sum"),
            ReduceOperation::Average => write!(f, "Average"),
            ReduceOperation::Max => write!(f, "Max"),
        }
    }
}

/// Simulates ring-allreduce across N nodes. Returns the reduced result.
pub fn ring_allreduce(node_data: &[Vec<f64>], op: ReduceOperation) -> Vec<f64> {
    if node_data.is_empty() {
        return Vec::new();
    }
    let len = node_data[0].len();
    let mut result = vec![0.0; len];

    match op {
        ReduceOperation::Sum => {
            for data in node_data {
                for (i, &v) in data.iter().enumerate() {
                    result[i] += v;
                }
            }
        }
        ReduceOperation::Average => {
            for data in node_data {
                for (i, &v) in data.iter().enumerate() {
                    result[i] += v;
                }
            }
            let n = node_data.len() as f64;
            for v in &mut result {
                *v /= n;
            }
        }
        ReduceOperation::Max => {
            result = node_data[0].clone();
            for data in &node_data[1..] {
                for (i, &v) in data.iter().enumerate() {
                    if v > result[i] {
                        result[i] = v;
                    }
                }
            }
        }
    }
    result
}

/// Returns the number of communication steps in ring-allreduce.
pub fn ring_allreduce_comm_steps(num_nodes: usize) -> usize {
    if num_nodes <= 1 {
        0
    } else {
        2 * (num_nodes - 1) // scatter-reduce + allgather
    }
}

/// Returns the per-step data volume for ring-allreduce.
pub fn ring_allreduce_per_step_bytes(total_bytes: usize, num_nodes: usize) -> usize {
    if num_nodes <= 1 {
        0
    } else {
        total_bytes / num_nodes
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D4.9: Pipeline Parallelism
// ═══════════════════════════════════════════════════════════════════════

/// A pipeline stage in model-parallel execution.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Stage index (0-based).
    pub index: usize,
    /// Assigned node.
    pub node: DataNodeId,
    /// Stage name (e.g., layer names).
    pub name: String,
    /// Micro-batch queue (simulated).
    pub micro_batches: Vec<Vec<f64>>,
}

/// Pipeline parallelism configuration.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Number of pipeline stages.
    pub num_stages: usize,
    /// Number of micro-batches.
    pub num_micro_batches: usize,
    /// Stages.
    pub stages: Vec<PipelineStage>,
}

impl PipelineConfig {
    /// Creates a new pipeline configuration.
    pub fn new(stage_names: &[&str], nodes: &[DataNodeId], num_micro_batches: usize) -> Self {
        let stages = stage_names
            .iter()
            .enumerate()
            .map(|(i, name)| PipelineStage {
                index: i,
                node: nodes[i % nodes.len()],
                name: name.to_string(),
                micro_batches: Vec::new(),
            })
            .collect();

        PipelineConfig {
            num_stages: stage_names.len(),
            num_micro_batches,
            stages,
        }
    }

    /// Computes the pipeline fill time (number of steps before all stages are active).
    pub fn fill_time(&self) -> usize {
        self.num_stages - 1
    }

    /// Computes the total pipeline steps for all micro-batches.
    pub fn total_steps(&self) -> usize {
        self.num_stages + self.num_micro_batches - 1
    }

    /// Computes pipeline efficiency (ideal time / actual time).
    pub fn efficiency(&self) -> f64 {
        if self.total_steps() == 0 {
            return 0.0;
        }
        self.num_micro_batches as f64 / self.total_steps() as f64
    }

    /// Returns which stages are active at a given time step.
    pub fn active_stages(&self, step: usize) -> Vec<usize> {
        (0..self.num_stages)
            .filter(|&stage| step >= stage && (step - stage) < self.num_micro_batches)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D4.10: Data Plane Statistics
// ═══════════════════════════════════════════════════════════════════════

/// Aggregate statistics for data plane operations.
#[derive(Debug, Clone, Default)]
pub struct DataPlaneStats {
    /// Total bytes transferred.
    pub bytes_transferred: u64,
    /// Total messages sent.
    pub messages_sent: u64,
    /// Total partitions created.
    pub partitions_created: u64,
    /// Total allreduce operations.
    pub allreduce_ops: u64,
    /// Total scatter operations.
    pub scatter_ops: u64,
    /// Total gather operations.
    pub gather_ops: u64,
    /// Total broadcast operations.
    pub broadcast_ops: u64,
}

impl DataPlaneStats {
    /// Creates new empty stats.
    pub fn new() -> Self {
        DataPlaneStats::default()
    }

    /// Records a transfer.
    pub fn record_transfer(&mut self, bytes: u64) {
        self.bytes_transferred += bytes;
        self.messages_sent += 1;
    }

    /// Returns throughput in bytes per operation.
    pub fn throughput_per_op(&self) -> f64 {
        if self.messages_sent == 0 {
            return 0.0;
        }
        self.bytes_transferred as f64 / self.messages_sent as f64
    }
}

impl fmt::Display for DataPlaneStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DataPlane(bytes={}, msgs={}, parts={})",
            self.bytes_transferred, self.messages_sent, self.partitions_created
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(ids: &[u64]) -> Vec<DataNodeId> {
        ids.iter().map(|&id| DataNodeId(id)).collect()
    }

    // D4.1 — Data Partitioning
    #[test]
    fn d4_1_partition_by_rows() {
        let data: Vec<f64> = (0..12).map(|x| x as f64).collect();
        let parts = partition_by_rows(&data, (4, 3), 2, &nodes(&[0, 1]));
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].shape, (2, 3));
        assert_eq!(parts[1].shape, (2, 3));
        assert_eq!(parts[0].data, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(parts[1].data, vec![6.0, 7.0, 8.0, 9.0, 10.0, 11.0]);
    }

    #[test]
    fn d4_1_partition_by_columns() {
        let data: Vec<f64> = (0..12).map(|x| x as f64).collect();
        let parts = partition_by_columns(&data, (3, 4), 2, &nodes(&[0, 1]));
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].shape, (3, 2));
        assert_eq!(parts[1].shape, (3, 2));
        // First partition: columns 0,1 for each row.
        assert_eq!(parts[0].data, vec![0.0, 1.0, 4.0, 5.0, 8.0, 9.0]);
    }

    #[test]
    fn d4_1_partition_strategy_display() {
        assert_eq!(PartitionStrategy::ByRows.to_string(), "ByRows");
        assert_eq!(
            PartitionStrategy::Hash { key_column: 2 }.to_string(),
            "Hash(col=2)"
        );
    }

    // D4.2 — Transfer Protocol
    #[test]
    fn d4_2_transfer_log() {
        let mut log = TransferLog::new();
        log.record(TransferMessage {
            msg_type: TransferType::Push,
            source: DataNodeId(0),
            destination: DataNodeId(1),
            partition_id: PartitionId(0),
            seq: 1,
            payload: vec![1, 2, 3, 4],
        });
        log.record(TransferMessage {
            msg_type: TransferType::Ack,
            source: DataNodeId(1),
            destination: DataNodeId(0),
            partition_id: PartitionId(0),
            seq: 2,
            payload: vec![],
        });
        assert_eq!(log.total_messages(), 2);
        assert_eq!(log.total_bytes(), 4);
        assert_eq!(log.messages_for(DataNodeId(1)).len(), 1);
    }

    #[test]
    fn d4_2_transfer_type_display() {
        assert_eq!(TransferType::Push.to_string(), "Push");
        assert_eq!(TransferType::Nack.to_string(), "Nack");
    }

    // D4.3 — Serialization
    #[test]
    fn d4_3_serialize_deserialize_frame() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let frame = serialize_frame(&data, (2, 3), false);
        let (header, decoded) = deserialize_frame(&frame).unwrap();
        assert_eq!(header.shape, (2, 3));
        assert_eq!(header.compressed, false);
        assert_eq!(decoded, data);
    }

    #[test]
    fn d4_3_serialize_with_compression() {
        let data = vec![1.0, 2.0, 3.0];
        let frame = serialize_frame(&data, (1, 3), true);
        let (header, decoded) = deserialize_frame(&frame).unwrap();
        assert!(header.compressed);
        assert_eq!(decoded, data);
    }

    #[test]
    fn d4_3_data_type_display() {
        assert_eq!(DataType::F64.to_string(), "f64");
        assert_eq!(DataType::I32.to_string(), "i32");
    }

    // D4.4 — Compression
    #[test]
    fn d4_4_lz4_roundtrip() {
        let original = b"hello world data plane test data";
        let compressed = lz4_compress(original);
        let decompressed = lz4_decompress(&compressed);
        assert_eq!(decompressed, original);
    }

    #[test]
    fn d4_4_compression_ratio() {
        let ratio = compression_ratio(1000, 800);
        assert!((ratio - 0.8).abs() < 1e-10);
        assert!((compression_ratio(0, 0) - 1.0).abs() < 1e-10);
    }

    // D4.5 — Scatter
    #[test]
    fn d4_5_scatter() {
        let data: Vec<f64> = (0..9).map(|x| x as f64).collect();
        let targets = nodes(&[0, 1, 2]);
        let parts = scatter(&data, (3, 3), DataNodeId(0), &targets);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].node, DataNodeId(0));
        assert_eq!(parts[1].node, DataNodeId(1));
        assert_eq!(parts[2].node, DataNodeId(2));
    }

    #[test]
    fn d4_5_scatter_sizes() {
        let sizes = scatter_sizes(10, 3);
        assert_eq!(sizes, vec![4, 3, 3]); // 10/3 = 3 remainder 1
    }

    // D4.6 — Gather
    #[test]
    fn d4_6_gather() {
        let data: Vec<f64> = (0..12).map(|x| x as f64).collect();
        let parts = partition_by_rows(&data, (4, 3), 2, &nodes(&[0, 1]));
        let gathered = gather(&parts, (4, 3));
        assert_eq!(gathered, data);
    }

    #[test]
    fn d4_6_validate_gather() {
        let parts = vec![
            DataPartition {
                id: PartitionId(0),
                node: DataNodeId(0),
                shape: (2, 3),
                data: vec![0.0; 6],
            },
            DataPartition {
                id: PartitionId(1),
                node: DataNodeId(1),
                shape: (2, 3),
                data: vec![0.0; 6],
            },
        ];
        assert!(validate_gather(&parts, (4, 3)));
        assert!(!validate_gather(&parts, (5, 3)));
    }

    // D4.7 — Broadcast
    #[test]
    fn d4_7_broadcast() {
        let data = vec![1.0, 2.0, 3.0];
        let targets = nodes(&[1, 2, 3]);
        let parts = broadcast(&data, (1, 3), DataNodeId(0), &targets);
        assert_eq!(parts.len(), 3);
        for part in &parts {
            assert_eq!(part.data, data);
        }
    }

    #[test]
    fn d4_7_broadcast_bandwidth() {
        assert_eq!(broadcast_bandwidth(1024, 4), 4096);
    }

    // D4.8 — Ring-AllReduce
    #[test]
    fn d4_8_ring_allreduce_sum() {
        let node_data = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        let result = ring_allreduce(&node_data, ReduceOperation::Sum);
        assert_eq!(result, vec![12.0, 15.0, 18.0]);
    }

    #[test]
    fn d4_8_ring_allreduce_average() {
        let node_data = vec![vec![2.0, 4.0], vec![6.0, 8.0]];
        let result = ring_allreduce(&node_data, ReduceOperation::Average);
        assert_eq!(result, vec![4.0, 6.0]);
    }

    #[test]
    fn d4_8_ring_allreduce_comm_steps() {
        assert_eq!(ring_allreduce_comm_steps(1), 0);
        assert_eq!(ring_allreduce_comm_steps(4), 6);
        assert_eq!(ring_allreduce_comm_steps(8), 14);
    }

    #[test]
    fn d4_8_ring_allreduce_per_step_bytes() {
        assert_eq!(ring_allreduce_per_step_bytes(1000, 4), 250);
    }

    // D4.9 — Pipeline Parallelism
    #[test]
    fn d4_9_pipeline_config() {
        let ns = nodes(&[0, 1, 2]);
        let config = PipelineConfig::new(&["embed", "transformer", "head"], &ns, 4);
        assert_eq!(config.num_stages, 3);
        assert_eq!(config.fill_time(), 2);
        assert_eq!(config.total_steps(), 6); // 3 + 4 - 1
    }

    #[test]
    fn d4_9_pipeline_efficiency() {
        let ns = nodes(&[0, 1, 2, 3]);
        let config = PipelineConfig::new(&["s1", "s2", "s3", "s4"], &ns, 8);
        // total_steps = 4 + 8 - 1 = 11
        // efficiency = 8/11 ≈ 0.727
        let eff = config.efficiency();
        assert!(eff > 0.7 && eff < 0.75);
    }

    #[test]
    fn d4_9_pipeline_active_stages() {
        let ns = nodes(&[0, 1, 2]);
        let config = PipelineConfig::new(&["a", "b", "c"], &ns, 3);
        // Step 0: only stage 0 active.
        assert_eq!(config.active_stages(0), vec![0]);
        // Step 1: stages 0, 1.
        assert_eq!(config.active_stages(1), vec![0, 1]);
        // Step 2: all stages.
        assert_eq!(config.active_stages(2), vec![0, 1, 2]);
    }

    // D4.10 — Statistics
    #[test]
    fn d4_10_data_plane_stats() {
        let mut stats = DataPlaneStats::new();
        stats.record_transfer(1024);
        stats.record_transfer(2048);
        assert_eq!(stats.bytes_transferred, 3072);
        assert_eq!(stats.messages_sent, 2);
        assert!((stats.throughput_per_op() - 1536.0).abs() < 1e-10);
    }

    #[test]
    fn d4_10_stats_display() {
        let stats = DataPlaneStats {
            bytes_transferred: 100,
            messages_sent: 5,
            partitions_created: 3,
            ..Default::default()
        };
        let s = stats.to_string();
        assert!(s.contains("bytes=100"));
        assert!(s.contains("msgs=5"));
    }

    #[test]
    fn d4_10_reduce_operation_display() {
        assert_eq!(ReduceOperation::Sum.to_string(), "Sum");
        assert_eq!(ReduceOperation::Average.to_string(), "Average");
        assert_eq!(ReduceOperation::Max.to_string(), "Max");
    }

    #[test]
    fn d4_10_partition_id_display() {
        assert_eq!(PartitionId(5).to_string(), "Part(5)");
        assert_eq!(DataNodeId(3).to_string(), "DNode(3)");
    }
}
