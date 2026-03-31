//! Distributed ML Training — data-parallel, model-parallel, gradient sync,
//! parameter server, LR scaling, checkpoint, mixed precision, elastic training.
//!
//! Sprint D5: Distributed ML (10 tasks)
//! All simulated — no real networking or GPU.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D5.1: Data-Parallel Training
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a training worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrainWorkerId(pub u64);

impl fmt::Display for TrainWorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TWorker({})", self.0)
    }
}

/// Configuration for data-parallel training.
#[derive(Debug, Clone)]
pub struct DataParallelConfig {
    /// Number of workers.
    pub num_workers: usize,
    /// Global batch size.
    pub global_batch_size: usize,
    /// Whether to synchronize gradients after each step.
    pub sync_every_step: bool,
    /// Communication backend name.
    pub comm_backend: String,
}

impl DataParallelConfig {
    /// Creates a new data-parallel configuration.
    pub fn new(num_workers: usize, global_batch_size: usize) -> Self {
        DataParallelConfig {
            num_workers,
            global_batch_size,
            sync_every_step: true,
            comm_backend: "ring-allreduce".to_string(),
        }
    }

    /// Returns the per-worker (local) batch size.
    pub fn local_batch_size(&self) -> usize {
        self.global_batch_size / self.num_workers
    }

    /// Returns the effective batch size (accounts for all workers).
    pub fn effective_batch_size(&self) -> usize {
        self.local_batch_size() * self.num_workers
    }
}

/// Simulated data-parallel training state.
#[derive(Debug)]
pub struct DataParallelTrainer {
    /// Configuration.
    pub config: DataParallelConfig,
    /// Current model parameters (shared across all workers).
    pub params: Vec<f64>,
    /// Per-worker local gradients.
    pub worker_gradients: HashMap<TrainWorkerId, Vec<f64>>,
    /// Global step counter.
    pub global_step: u64,
    /// Learning rate.
    pub learning_rate: f64,
}

impl DataParallelTrainer {
    /// Creates a new data-parallel trainer.
    pub fn new(config: DataParallelConfig, initial_params: Vec<f64>, lr: f64) -> Self {
        DataParallelTrainer {
            config,
            params: initial_params,
            worker_gradients: HashMap::new(),
            global_step: 0,
            learning_rate: lr,
        }
    }

    /// Records local gradients from a worker.
    pub fn submit_gradients(&mut self, worker: TrainWorkerId, gradients: Vec<f64>) {
        self.worker_gradients.insert(worker, gradients);
    }

    /// Synchronizes gradients from all workers (allreduce average).
    pub fn sync_gradients(&mut self) -> Option<Vec<f64>> {
        if self.worker_gradients.len() < self.config.num_workers {
            return None; // Not all workers have submitted yet.
        }

        let param_count = self.params.len();
        let mut avg_grad = vec![0.0; param_count];
        let n = self.worker_gradients.len() as f64;

        for grad in self.worker_gradients.values() {
            for (i, &g) in grad.iter().enumerate().take(param_count) {
                avg_grad[i] += g / n;
            }
        }

        self.worker_gradients.clear();
        Some(avg_grad)
    }

    /// Applies averaged gradients to model parameters (SGD step).
    pub fn step(&mut self, gradients: &[f64]) {
        for (p, g) in self.params.iter_mut().zip(gradients.iter()) {
            *p -= self.learning_rate * g;
        }
        self.global_step += 1;
    }

    /// Runs a full sync + step cycle. Returns true if a step was taken.
    pub fn sync_and_step(&mut self) -> bool {
        if let Some(avg_grad) = self.sync_gradients() {
            self.step(&avg_grad);
            true
        } else {
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.2: Gradient Sync (AllReduce)
// ═══════════════════════════════════════════════════════════════════════

/// Gradient synchronization strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradSyncStrategy {
    /// Tree-reduce + broadcast.
    TreeAllReduce,
    /// Ring-based allreduce.
    RingAllReduce,
    /// Parameter server (centralized).
    ParameterServer,
    /// Decentralized gossip averaging.
    GossipAveraging,
}

impl fmt::Display for GradSyncStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GradSyncStrategy::TreeAllReduce => write!(f, "TreeAllReduce"),
            GradSyncStrategy::RingAllReduce => write!(f, "RingAllReduce"),
            GradSyncStrategy::ParameterServer => write!(f, "ParameterServer"),
            GradSyncStrategy::GossipAveraging => write!(f, "GossipAveraging"),
        }
    }
}

/// Performs allreduce-sum on gradient vectors from all workers.
pub fn allreduce_sum(gradients: &[Vec<f64>]) -> Vec<f64> {
    if gradients.is_empty() {
        return Vec::new();
    }
    let len = gradients[0].len();
    let mut result = vec![0.0; len];
    for grad in gradients {
        for (i, &g) in grad.iter().enumerate().take(len) {
            result[i] += g;
        }
    }
    result
}

/// Performs allreduce-average on gradient vectors.
pub fn allreduce_average(gradients: &[Vec<f64>]) -> Vec<f64> {
    let sum = allreduce_sum(gradients);
    let n = gradients.len() as f64;
    sum.into_iter().map(|v| v / n).collect()
}

/// Estimates communication cost for allreduce (bytes transferred per node).
pub fn allreduce_comm_cost(
    param_count: usize,
    num_workers: usize,
    bytes_per_param: usize,
) -> usize {
    // Ring allreduce: 2 * (N-1) / N * total_bytes ≈ 2 * total_bytes for large N.
    let total_bytes = param_count * bytes_per_param;
    if num_workers <= 1 {
        0
    } else {
        2 * (num_workers - 1) * total_bytes / num_workers
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.3: Model-Parallel Training
// ═══════════════════════════════════════════════════════════════════════

/// Model parallelism: partition model layers across workers.
#[derive(Debug, Clone)]
pub struct ModelParallelConfig {
    /// Number of pipeline stages.
    pub num_stages: usize,
    /// Worker assignment per stage.
    pub stage_workers: Vec<TrainWorkerId>,
    /// Layer names per stage.
    pub stage_layers: Vec<Vec<String>>,
    /// Number of micro-batches for pipeline.
    pub micro_batches: usize,
}

impl ModelParallelConfig {
    /// Creates a model-parallel configuration by splitting layers evenly.
    pub fn even_split(layers: &[&str], workers: &[TrainWorkerId], micro_batches: usize) -> Self {
        let per_stage = layers.len() / workers.len();
        let mut stage_layers = Vec::new();
        for chunk in layers.chunks(per_stage) {
            stage_layers.push(chunk.iter().map(|s| s.to_string()).collect());
        }
        // Merge leftover into last stage.
        if stage_layers.len() > workers.len() {
            let last_extra: Vec<String> = stage_layers.pop().unwrap_or_default();
            if let Some(last) = stage_layers.last_mut() {
                last.extend(last_extra);
            }
        }

        ModelParallelConfig {
            num_stages: workers.len(),
            stage_workers: workers.to_vec(),
            stage_layers,
            micro_batches,
        }
    }

    /// Returns pipeline bubble ratio (idle / total).
    pub fn bubble_ratio(&self) -> f64 {
        let total = self.num_stages + self.micro_batches - 1;
        if total == 0 {
            return 0.0;
        }
        let bubble = self.num_stages - 1;
        bubble as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.4: Parameter Server
// ═══════════════════════════════════════════════════════════════════════

/// Centralized parameter server for distributed training.
#[derive(Debug)]
pub struct DistributedParamServer {
    /// Model parameters.
    pub params: Vec<f64>,
    /// Learning rate.
    pub learning_rate: f64,
    /// Momentum buffer (for SGD with momentum).
    pub momentum_buffer: Vec<f64>,
    /// Momentum coefficient.
    pub momentum: f64,
    /// Number of gradient updates applied.
    pub update_count: u64,
    /// Per-worker gradient accumulator (for async SGD).
    pending_gradients: HashMap<TrainWorkerId, Vec<f64>>,
    /// Required number of workers before applying.
    pub sync_barrier: usize,
}

impl DistributedParamServer {
    /// Creates a new parameter server.
    pub fn new(params: Vec<f64>, lr: f64, momentum: f64, sync_barrier: usize) -> Self {
        let len = params.len();
        DistributedParamServer {
            params,
            learning_rate: lr,
            momentum_buffer: vec![0.0; len],
            momentum,
            update_count: 0,
            pending_gradients: HashMap::new(),
            sync_barrier,
        }
    }

    /// Pushes gradients from a worker. Returns true if barrier is reached.
    pub fn push_gradients(&mut self, worker: TrainWorkerId, gradients: Vec<f64>) -> bool {
        self.pending_gradients.insert(worker, gradients);
        self.pending_gradients.len() >= self.sync_barrier
    }

    /// Applies accumulated gradients (averaged) with optional momentum.
    pub fn apply(&mut self) -> bool {
        if self.pending_gradients.len() < self.sync_barrier {
            return false;
        }

        let n = self.pending_gradients.len() as f64;
        let param_count = self.params.len();
        let mut avg_grad = vec![0.0; param_count];

        for grad in self.pending_gradients.values() {
            for (i, &g) in grad.iter().enumerate().take(param_count) {
                avg_grad[i] += g / n;
            }
        }

        // SGD with momentum.
        for ((p, mb), &g) in self
            .params
            .iter_mut()
            .zip(self.momentum_buffer.iter_mut())
            .zip(avg_grad.iter())
        {
            *mb = self.momentum * *mb + g;
            *p -= self.learning_rate * *mb;
        }

        self.update_count += 1;
        self.pending_gradients.clear();
        true
    }

    /// Returns a snapshot of current parameters.
    pub fn get_params(&self) -> &[f64] {
        &self.params
    }

    /// Returns the number of pending worker gradients.
    pub fn pending_count(&self) -> usize {
        self.pending_gradients.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.5: Learning Rate Scaling
// ═══════════════════════════════════════════════════════════════════════

/// LR scaling rule for distributed training.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LrScalingRule {
    /// Linear scaling: lr * num_workers.
    Linear,
    /// Square root scaling: lr * sqrt(num_workers).
    SquareRoot,
    /// No scaling.
    None,
}

impl fmt::Display for LrScalingRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LrScalingRule::Linear => write!(f, "Linear"),
            LrScalingRule::SquareRoot => write!(f, "SquareRoot"),
            LrScalingRule::None => write!(f, "None"),
        }
    }
}

/// Computes the scaled learning rate.
pub fn scale_lr(base_lr: f64, num_workers: usize, rule: &LrScalingRule) -> f64 {
    match rule {
        LrScalingRule::Linear => base_lr * num_workers as f64,
        LrScalingRule::SquareRoot => base_lr * (num_workers as f64).sqrt(),
        LrScalingRule::None => base_lr,
    }
}

/// Warmup scheduler: linearly increases LR from 0 to target over N steps.
#[derive(Debug, Clone)]
pub struct WarmupScheduler {
    /// Target learning rate.
    pub target_lr: f64,
    /// Number of warmup steps.
    pub warmup_steps: u64,
}

impl WarmupScheduler {
    /// Creates a new warmup scheduler.
    pub fn new(target_lr: f64, warmup_steps: u64) -> Self {
        WarmupScheduler {
            target_lr,
            warmup_steps,
        }
    }

    /// Returns the learning rate at the given step.
    pub fn lr_at_step(&self, step: u64) -> f64 {
        if step >= self.warmup_steps || self.warmup_steps == 0 {
            self.target_lr
        } else {
            self.target_lr * (step as f64 / self.warmup_steps as f64)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.6: Checkpoint Save/Load
// ═══════════════════════════════════════════════════════════════════════

/// A training checkpoint.
#[derive(Debug, Clone)]
pub struct TrainingCheckpoint {
    /// Global step at checkpoint time.
    pub global_step: u64,
    /// Epoch number.
    pub epoch: u64,
    /// Model parameters.
    pub params: Vec<f64>,
    /// Optimizer state (momentum buffer, etc.).
    pub optimizer_state: Vec<f64>,
    /// Learning rate at checkpoint.
    pub learning_rate: f64,
    /// Training loss at checkpoint.
    pub loss: f64,
}

/// Checkpoint manager for distributed training.
#[derive(Debug)]
pub struct TrainingCheckpointManager {
    /// All saved checkpoints.
    checkpoints: Vec<TrainingCheckpoint>,
    /// Maximum checkpoints to retain.
    pub max_retained: usize,
    /// Whether to save only the best (lowest loss).
    pub save_best_only: bool,
    /// Best loss seen so far.
    pub best_loss: f64,
}

impl TrainingCheckpointManager {
    /// Creates a new checkpoint manager.
    pub fn new(max_retained: usize, save_best_only: bool) -> Self {
        TrainingCheckpointManager {
            checkpoints: Vec::new(),
            max_retained,
            save_best_only,
            best_loss: f64::INFINITY,
        }
    }

    /// Saves a checkpoint. Returns true if it was actually saved.
    pub fn save(&mut self, checkpoint: TrainingCheckpoint) -> bool {
        if self.save_best_only && checkpoint.loss >= self.best_loss {
            return false;
        }

        if checkpoint.loss < self.best_loss {
            self.best_loss = checkpoint.loss;
        }

        self.checkpoints.push(checkpoint);

        // Trim old checkpoints.
        while self.checkpoints.len() > self.max_retained {
            self.checkpoints.remove(0);
        }
        true
    }

    /// Loads the latest checkpoint.
    pub fn load_latest(&self) -> Option<&TrainingCheckpoint> {
        self.checkpoints.last()
    }

    /// Loads the best checkpoint (lowest loss).
    pub fn load_best(&self) -> Option<&TrainingCheckpoint> {
        self.checkpoints.iter().min_by(|a, b| {
            a.loss
                .partial_cmp(&b.loss)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Returns the number of saved checkpoints.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.7: Mixed Precision Training
// ═══════════════════════════════════════════════════════════════════════

/// Precision mode for training.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Full 32-bit float.
    FP32,
    /// Half 16-bit float.
    FP16,
    /// BFloat16.
    BF16,
    /// Mixed: forward in FP16, backward in FP32.
    Mixed,
}

impl fmt::Display for Precision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Precision::FP32 => write!(f, "FP32"),
            Precision::FP16 => write!(f, "FP16"),
            Precision::BF16 => write!(f, "BF16"),
            Precision::Mixed => write!(f, "Mixed"),
        }
    }
}

/// Gradient scaler for mixed precision training (prevents underflow in FP16).
#[derive(Debug, Clone)]
pub struct GradScaler {
    /// Current scale factor.
    pub scale: f64,
    /// Growth factor (multiply scale after N consecutive good steps).
    pub growth_factor: f64,
    /// Backoff factor (reduce scale after overflow).
    pub backoff_factor: f64,
    /// Steps since last overflow.
    pub growth_interval: u32,
    /// Current count of good steps.
    pub good_steps: u32,
    /// Whether scaling is enabled.
    pub enabled: bool,
}

impl GradScaler {
    /// Creates a new gradient scaler.
    pub fn new(initial_scale: f64) -> Self {
        GradScaler {
            scale: initial_scale,
            growth_factor: 2.0,
            backoff_factor: 0.5,
            growth_interval: 2000,
            good_steps: 0,
            enabled: true,
        }
    }

    /// Scales gradients before backward pass.
    pub fn scale_gradients(&self, gradients: &[f64]) -> Vec<f64> {
        if !self.enabled {
            return gradients.to_vec();
        }
        gradients.iter().map(|&g| g * self.scale).collect()
    }

    /// Unscales gradients before optimizer step.
    pub fn unscale_gradients(&self, gradients: &[f64]) -> Vec<f64> {
        if !self.enabled || self.scale == 0.0 {
            return gradients.to_vec();
        }
        gradients.iter().map(|&g| g / self.scale).collect()
    }

    /// Checks for overflow (inf/nan) in gradients.
    pub fn check_overflow(&self, gradients: &[f64]) -> bool {
        gradients.iter().any(|&g| g.is_nan() || g.is_infinite())
    }

    /// Updates the scaler after a step. Returns true if the step was valid.
    pub fn update(&mut self, gradients: &[f64]) -> bool {
        if !self.enabled {
            return true;
        }

        if self.check_overflow(gradients) {
            // Overflow detected: reduce scale, skip step.
            self.scale *= self.backoff_factor;
            self.good_steps = 0;
            return false;
        }

        self.good_steps += 1;
        if self.good_steps >= self.growth_interval {
            self.scale *= self.growth_factor;
            self.good_steps = 0;
        }
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.8: Elastic Training (Add/Remove Workers)
// ═══════════════════════════════════════════════════════════════════════

/// An elastic training coordinator that supports dynamic worker scaling.
#[derive(Debug)]
pub struct ElasticTrainer {
    /// Active worker IDs.
    pub active_workers: Vec<TrainWorkerId>,
    /// Minimum workers required.
    pub min_workers: usize,
    /// Maximum workers allowed.
    pub max_workers: usize,
    /// Current global batch size (adjusts with worker count).
    pub global_batch_size: usize,
    /// Per-worker batch size (fixed).
    pub per_worker_batch: usize,
    /// Pending workers to add.
    pub pending_joins: Vec<TrainWorkerId>,
    /// Workers scheduled for removal.
    pub pending_leaves: Vec<TrainWorkerId>,
}

impl ElasticTrainer {
    /// Creates a new elastic trainer.
    pub fn new(
        initial_workers: Vec<TrainWorkerId>,
        per_worker_batch: usize,
        min_workers: usize,
        max_workers: usize,
    ) -> Self {
        let global_batch = per_worker_batch * initial_workers.len();
        ElasticTrainer {
            active_workers: initial_workers,
            min_workers,
            max_workers,
            global_batch_size: global_batch,
            per_worker_batch,
            pending_joins: Vec::new(),
            pending_leaves: Vec::new(),
        }
    }

    /// Requests to add a worker. Returns true if accepted.
    pub fn request_join(&mut self, worker: TrainWorkerId) -> bool {
        if self.active_workers.len() + self.pending_joins.len() >= self.max_workers {
            return false;
        }
        if !self.pending_joins.contains(&worker) && !self.active_workers.contains(&worker) {
            self.pending_joins.push(worker);
        }
        true
    }

    /// Requests to remove a worker. Returns true if accepted.
    pub fn request_leave(&mut self, worker: TrainWorkerId) -> bool {
        if self.active_workers.len() <= self.min_workers {
            return false;
        }
        if self.active_workers.contains(&worker) && !self.pending_leaves.contains(&worker) {
            self.pending_leaves.push(worker);
        }
        true
    }

    /// Commits pending membership changes at a step boundary.
    pub fn commit_changes(&mut self) {
        // Remove leaving workers.
        for w in self.pending_leaves.drain(..) {
            self.active_workers.retain(|&aw| aw != w);
        }
        // Add joining workers.
        for w in self.pending_joins.drain(..) {
            if !self.active_workers.contains(&w) {
                self.active_workers.push(w);
            }
        }
        // Recalculate global batch size.
        self.global_batch_size = self.per_worker_batch * self.active_workers.len();
    }

    /// Returns the current worker count.
    pub fn worker_count(&self) -> usize {
        self.active_workers.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D5.9: Training Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Distributed training metrics.
#[derive(Debug, Clone, Default)]
pub struct TrainingMetrics {
    /// Loss history per step.
    pub loss_history: Vec<f64>,
    /// Throughput (samples/sec) per step.
    pub throughput_history: Vec<f64>,
    /// Communication overhead (ms) per step.
    pub comm_overhead_ms: Vec<f64>,
    /// GPU utilization per step (0.0-1.0).
    pub gpu_utilization: Vec<f64>,
}

impl TrainingMetrics {
    /// Creates new empty metrics.
    pub fn new() -> Self {
        TrainingMetrics::default()
    }

    /// Records a training step's metrics.
    pub fn record_step(&mut self, loss: f64, throughput: f64, comm_ms: f64, gpu_util: f64) {
        self.loss_history.push(loss);
        self.throughput_history.push(throughput);
        self.comm_overhead_ms.push(comm_ms);
        self.gpu_utilization.push(gpu_util);
    }

    /// Returns the average loss over the last N steps.
    pub fn avg_loss(&self, last_n: usize) -> f64 {
        let n = last_n.min(self.loss_history.len());
        if n == 0 {
            return 0.0;
        }
        let sum: f64 = self.loss_history[self.loss_history.len() - n..]
            .iter()
            .sum();
        sum / n as f64
    }

    /// Returns the average throughput.
    pub fn avg_throughput(&self) -> f64 {
        if self.throughput_history.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.throughput_history.iter().sum();
        sum / self.throughput_history.len() as f64
    }

    /// Returns the total number of recorded steps.
    pub fn total_steps(&self) -> usize {
        self.loss_history.len()
    }

    /// Returns the scaling efficiency (throughput_N_workers / (N * throughput_1_worker)).
    pub fn scaling_efficiency(&self, single_worker_throughput: f64, num_workers: usize) -> f64 {
        let avg = self.avg_throughput();
        if single_worker_throughput <= 0.0 || num_workers == 0 {
            return 0.0;
        }
        avg / (single_worker_throughput * num_workers as f64)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn workers(ids: &[u64]) -> Vec<TrainWorkerId> {
        ids.iter().map(|&id| TrainWorkerId(id)).collect()
    }

    // D5.1 — Data-Parallel Training
    #[test]
    fn d5_1_data_parallel_config() {
        let config = DataParallelConfig::new(4, 256);
        assert_eq!(config.local_batch_size(), 64);
        assert_eq!(config.effective_batch_size(), 256);
    }

    #[test]
    fn d5_1_data_parallel_trainer_sync() {
        let config = DataParallelConfig::new(2, 32);
        let params = vec![1.0, 2.0, 3.0];
        let mut trainer = DataParallelTrainer::new(config, params, 0.1);

        trainer.submit_gradients(TrainWorkerId(0), vec![1.0, 0.0, 0.0]);
        trainer.submit_gradients(TrainWorkerId(1), vec![0.0, 1.0, 0.0]);

        assert!(trainer.sync_and_step());
        // Averaged grads: [0.5, 0.5, 0.0]
        // New params: [1.0 - 0.1*0.5, 2.0 - 0.1*0.5, 3.0] = [0.95, 1.95, 3.0]
        assert!((trainer.params[0] - 0.95).abs() < 1e-10);
        assert!((trainer.params[1] - 1.95).abs() < 1e-10);
        assert!((trainer.params[2] - 3.0).abs() < 1e-10);
        assert_eq!(trainer.global_step, 1);
    }

    #[test]
    fn d5_1_trainer_partial_grads_no_step() {
        let config = DataParallelConfig::new(3, 64);
        let mut trainer = DataParallelTrainer::new(config, vec![0.0], 0.01);
        trainer.submit_gradients(TrainWorkerId(0), vec![1.0]);
        assert!(!trainer.sync_and_step()); // Only 1/3 workers submitted.
    }

    // D5.2 — Gradient Sync
    #[test]
    fn d5_2_allreduce_sum() {
        let grads = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]];
        let result = allreduce_sum(&grads);
        assert_eq!(result, vec![9.0, 12.0]);
    }

    #[test]
    fn d5_2_allreduce_average() {
        let grads = vec![vec![2.0, 4.0], vec![4.0, 8.0]];
        let result = allreduce_average(&grads);
        assert_eq!(result, vec![3.0, 6.0]);
    }

    #[test]
    fn d5_2_allreduce_comm_cost() {
        // 1M params, 4 workers, 4 bytes/param (f32)
        let cost = allreduce_comm_cost(1_000_000, 4, 4);
        // 2 * (4-1) * 4M / 4 = 2 * 3 * 1M = 6M
        assert_eq!(cost, 6_000_000);
    }

    #[test]
    fn d5_2_grad_sync_strategy_display() {
        assert_eq!(GradSyncStrategy::RingAllReduce.to_string(), "RingAllReduce");
        assert_eq!(
            GradSyncStrategy::ParameterServer.to_string(),
            "ParameterServer"
        );
    }

    // D5.3 — Model-Parallel Training
    #[test]
    fn d5_3_model_parallel_even_split() {
        let layers = &["embed", "attn1", "ffn1", "attn2", "ffn2", "head"];
        let ws = workers(&[0, 1, 2]);
        let config = ModelParallelConfig::even_split(layers, &ws, 4);
        assert_eq!(config.num_stages, 3);
        assert_eq!(config.stage_layers[0], vec!["embed", "attn1"]);
        assert_eq!(config.stage_layers[1], vec!["ffn1", "attn2"]);
    }

    #[test]
    fn d5_3_model_parallel_bubble_ratio() {
        let ws = workers(&[0, 1, 2, 3]);
        let config = ModelParallelConfig::even_split(&["l1", "l2", "l3", "l4"], &ws, 8);
        // bubble = 3, total = 4+8-1 = 11, ratio = 3/11 ≈ 0.273
        let ratio = config.bubble_ratio();
        assert!(ratio > 0.27 && ratio < 0.28);
    }

    // D5.4 — Parameter Server
    #[test]
    fn d5_4_param_server_sync_sgd() {
        let mut ps = DistributedParamServer::new(vec![1.0, 2.0], 0.1, 0.0, 2);
        ps.push_gradients(TrainWorkerId(0), vec![2.0, 4.0]);
        let ready = ps.push_gradients(TrainWorkerId(1), vec![4.0, 6.0]);
        assert!(ready);

        ps.apply();
        // Avg grad: [3.0, 5.0]. With lr=0.1: [1.0-0.3, 2.0-0.5] = [0.7, 1.5]
        assert!((ps.params[0] - 0.7).abs() < 1e-10);
        assert!((ps.params[1] - 1.5).abs() < 1e-10);
    }

    #[test]
    fn d5_4_param_server_momentum() {
        let mut ps = DistributedParamServer::new(vec![0.0], 0.1, 0.9, 1);
        ps.push_gradients(TrainWorkerId(0), vec![1.0]);
        ps.apply();
        // momentum_buf = 0.9*0 + 1.0 = 1.0, param = 0 - 0.1*1.0 = -0.1
        assert!((ps.params[0] - (-0.1)).abs() < 1e-10);

        ps.push_gradients(TrainWorkerId(0), vec![1.0]);
        ps.apply();
        // momentum_buf = 0.9*1.0 + 1.0 = 1.9, param = -0.1 - 0.1*1.9 = -0.29
        assert!((ps.params[0] - (-0.29)).abs() < 1e-10);
    }

    // D5.5 — Learning Rate Scaling
    #[test]
    fn d5_5_lr_scaling_linear() {
        assert!((scale_lr(0.01, 4, &LrScalingRule::Linear) - 0.04).abs() < 1e-10);
    }

    #[test]
    fn d5_5_lr_scaling_sqrt() {
        let scaled = scale_lr(0.01, 4, &LrScalingRule::SquareRoot);
        assert!((scaled - 0.02).abs() < 1e-10);
    }

    #[test]
    fn d5_5_warmup_scheduler() {
        let scheduler = WarmupScheduler::new(0.1, 100);
        assert!((scheduler.lr_at_step(0) - 0.0).abs() < 1e-10);
        assert!((scheduler.lr_at_step(50) - 0.05).abs() < 1e-10);
        assert!((scheduler.lr_at_step(100) - 0.1).abs() < 1e-10);
        assert!((scheduler.lr_at_step(200) - 0.1).abs() < 1e-10);
    }

    #[test]
    fn d5_5_lr_scaling_display() {
        assert_eq!(LrScalingRule::Linear.to_string(), "Linear");
        assert_eq!(LrScalingRule::SquareRoot.to_string(), "SquareRoot");
    }

    // D5.6 — Checkpoint Save/Load
    #[test]
    fn d5_6_checkpoint_save_load() {
        let mut mgr = TrainingCheckpointManager::new(3, false);
        mgr.save(TrainingCheckpoint {
            global_step: 100,
            epoch: 1,
            params: vec![1.0, 2.0],
            optimizer_state: vec![0.0, 0.0],
            learning_rate: 0.01,
            loss: 0.5,
        });
        mgr.save(TrainingCheckpoint {
            global_step: 200,
            epoch: 2,
            params: vec![0.5, 1.5],
            optimizer_state: vec![0.1, 0.1],
            learning_rate: 0.01,
            loss: 0.3,
        });

        assert_eq!(mgr.checkpoint_count(), 2);
        let latest = mgr.load_latest().unwrap();
        assert_eq!(latest.global_step, 200);
        let best = mgr.load_best().unwrap();
        assert_eq!(best.loss, 0.3);
    }

    #[test]
    fn d5_6_checkpoint_save_best_only() {
        let mut mgr = TrainingCheckpointManager::new(5, true);
        assert!(mgr.save(TrainingCheckpoint {
            global_step: 1,
            epoch: 0,
            params: vec![],
            optimizer_state: vec![],
            learning_rate: 0.01,
            loss: 1.0,
        }));
        // Worse loss: should not save.
        assert!(!mgr.save(TrainingCheckpoint {
            global_step: 2,
            epoch: 0,
            params: vec![],
            optimizer_state: vec![],
            learning_rate: 0.01,
            loss: 1.5,
        }));
        // Better loss: should save.
        assert!(mgr.save(TrainingCheckpoint {
            global_step: 3,
            epoch: 0,
            params: vec![],
            optimizer_state: vec![],
            learning_rate: 0.01,
            loss: 0.5,
        }));
        assert_eq!(mgr.checkpoint_count(), 2);
    }

    // D5.7 — Mixed Precision
    #[test]
    fn d5_7_grad_scaler_scale_unscale() {
        let scaler = GradScaler::new(1024.0);
        let grads = vec![1.0, 2.0, 3.0];
        let scaled = scaler.scale_gradients(&grads);
        assert_eq!(scaled, vec![1024.0, 2048.0, 3072.0]);
        let unscaled = scaler.unscale_gradients(&scaled);
        assert!((unscaled[0] - 1.0).abs() < 1e-10);
        assert!((unscaled[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn d5_7_grad_scaler_overflow_detection() {
        let mut scaler = GradScaler::new(1024.0);
        let bad_grads = vec![f64::NAN, 1.0];
        assert!(scaler.check_overflow(&bad_grads));
        assert!(!scaler.update(&bad_grads));
        assert!((scaler.scale - 512.0).abs() < 1e-10); // backed off by 0.5
    }

    #[test]
    fn d5_7_precision_display() {
        assert_eq!(Precision::FP32.to_string(), "FP32");
        assert_eq!(Precision::Mixed.to_string(), "Mixed");
        assert_eq!(Precision::BF16.to_string(), "BF16");
    }

    // D5.8 — Elastic Training
    #[test]
    fn d5_8_elastic_add_worker() {
        let mut elastic = ElasticTrainer::new(workers(&[0, 1]), 32, 1, 4);
        assert_eq!(elastic.global_batch_size, 64);

        assert!(elastic.request_join(TrainWorkerId(2)));
        elastic.commit_changes();
        assert_eq!(elastic.worker_count(), 3);
        assert_eq!(elastic.global_batch_size, 96);
    }

    #[test]
    fn d5_8_elastic_remove_worker() {
        let mut elastic = ElasticTrainer::new(workers(&[0, 1, 2]), 32, 1, 4);
        assert!(elastic.request_leave(TrainWorkerId(2)));
        elastic.commit_changes();
        assert_eq!(elastic.worker_count(), 2);
        assert_eq!(elastic.global_batch_size, 64);
    }

    #[test]
    fn d5_8_elastic_max_workers_limit() {
        let mut elastic = ElasticTrainer::new(workers(&[0, 1, 2, 3]), 32, 1, 4);
        assert!(!elastic.request_join(TrainWorkerId(4))); // already at max
    }

    #[test]
    fn d5_8_elastic_min_workers_limit() {
        let mut elastic = ElasticTrainer::new(workers(&[0]), 32, 1, 4);
        assert!(!elastic.request_leave(TrainWorkerId(0))); // at min
    }

    // D5.9 — Training Metrics
    #[test]
    fn d5_9_training_metrics() {
        let mut metrics = TrainingMetrics::new();
        metrics.record_step(1.0, 100.0, 5.0, 0.9);
        metrics.record_step(0.8, 120.0, 4.5, 0.92);
        metrics.record_step(0.6, 110.0, 5.5, 0.88);

        assert_eq!(metrics.total_steps(), 3);
        assert!((metrics.avg_loss(2) - 0.7).abs() < 1e-10);
        assert!((metrics.avg_throughput() - 110.0).abs() < 1e-10);
    }

    #[test]
    fn d5_9_scaling_efficiency() {
        let mut metrics = TrainingMetrics::new();
        // Single worker: 100 samples/sec. With 4 workers: 350 samples/sec.
        metrics.record_step(0.5, 350.0, 10.0, 0.85);
        let eff = metrics.scaling_efficiency(100.0, 4);
        // 350 / (100 * 4) = 0.875
        assert!((eff - 0.875).abs() < 1e-10);
    }

    // D5.10 — Integration
    #[test]
    fn d5_10_worker_id_display() {
        assert_eq!(TrainWorkerId(42).to_string(), "TWorker(42)");
    }

    #[test]
    fn d5_10_full_training_cycle() {
        // End-to-end: 2-worker data-parallel training with checkpoint.
        let config = DataParallelConfig::new(2, 64);
        let mut trainer = DataParallelTrainer::new(config, vec![0.5, -0.3], 0.01);
        let mut ckpt_mgr = TrainingCheckpointManager::new(3, false);
        let mut metrics = TrainingMetrics::new();

        // Simulate 3 training steps.
        for step in 0..3 {
            trainer.submit_gradients(TrainWorkerId(0), vec![0.1 * (step as f64 + 1.0), -0.2]);
            trainer.submit_gradients(TrainWorkerId(1), vec![0.3, 0.1 * (step as f64 + 1.0)]);
            assert!(trainer.sync_and_step());

            let loss = 1.0 / (step as f64 + 1.0);
            metrics.record_step(loss, 200.0, 5.0, 0.9);

            ckpt_mgr.save(TrainingCheckpoint {
                global_step: trainer.global_step,
                epoch: 0,
                params: trainer.params.clone(),
                optimizer_state: vec![],
                learning_rate: trainer.learning_rate,
                loss,
            });
        }

        assert_eq!(trainer.global_step, 3);
        assert_eq!(ckpt_mgr.checkpoint_count(), 3);
        assert_eq!(metrics.total_steps(), 3);

        // Verify parameters changed.
        assert!(trainer.params[0] != 0.5 || trainer.params[1] != -0.3);
    }
}
