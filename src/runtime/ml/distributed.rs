//! Distributed training simulation — parameter server, workers, and aggregation.
//!
//! Implements simulated distributed training with parameter server architecture,
//! data sharding, gradient aggregation strategies, and synchronous/asynchronous modes.
//! No external RPC dependencies — all communication is in-process simulation.

use ndarray::Array2;
use std::collections::HashMap;

use super::tensor::TensorError;

// ═══════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Role of a process in the distributed training setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Parameter server: holds master weights, aggregates gradients.
    ParameterServer,
    /// Worker: computes gradients on a data shard.
    Worker,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::ParameterServer => write!(f, "ParameterServer"),
            Role::Worker => write!(f, "Worker"),
        }
    }
}

/// Gradient aggregation strategy.
#[derive(Debug, Clone, PartialEq)]
pub enum AggregationStrategy {
    /// Average all gradients (divide sum by worker count).
    AllReduceMean,
    /// Sum all gradients without averaging.
    AllReduceSum,
    /// Sparse gradient: keep only top K% of gradients by magnitude.
    TopK(f64),
}

impl AggregationStrategy {
    /// Returns a human-readable name.
    pub fn name(&self) -> &str {
        match self {
            AggregationStrategy::AllReduceMean => "AllReduceMean",
            AggregationStrategy::AllReduceSum => "AllReduceSum",
            AggregationStrategy::TopK(_) => "TopK",
        }
    }
}

/// Training synchronization mode.
#[derive(Debug, Clone, PartialEq)]
pub enum TrainingMode {
    /// All workers must synchronize before parameter update.
    Synchronous,
    /// Workers push/pull independently; server updates after each push.
    Asynchronous {
        /// Maximum allowed staleness (number of steps a worker can be behind).
        max_staleness: usize,
    },
}

/// Configuration for distributed training.
#[derive(Debug, Clone)]
pub struct DistributedConfig {
    /// Role of this process.
    pub role: Role,
    /// Total number of processes in the world.
    pub world_size: usize,
    /// Rank (ID) of this process (0-indexed).
    pub rank: usize,
    /// Server address (for display/logging only).
    pub server_addr: String,
}

impl DistributedConfig {
    /// Creates a config for a parameter server.
    pub fn server(world_size: usize) -> Self {
        Self {
            role: Role::ParameterServer,
            world_size,
            rank: 0,
            server_addr: "localhost:50051".to_string(),
        }
    }

    /// Creates a config for a worker with the given rank.
    pub fn worker(rank: usize, world_size: usize) -> Self {
        Self {
            role: Role::Worker,
            world_size,
            rank,
            server_addr: "localhost:50051".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Parameter Server
// ═══════════════════════════════════════════════════════════════════════

/// Parameter server that holds master weights and aggregates gradients.
///
/// In the simulation, all workers interact with the server in-process.
#[derive(Debug, Clone)]
pub struct ParameterServer {
    /// Master weights, keyed by parameter name.
    weights: HashMap<String, Array2<f64>>,
    /// Gradient accumulator: name → list of per-worker gradients.
    grad_accumulator: HashMap<String, Vec<Array2<f64>>>,
    /// Number of workers expected.
    worker_count: usize,
    /// Barrier state: set of worker ranks that have reached the barrier.
    barrier_arrived: Vec<bool>,
    /// Aggregation strategy.
    strategy: AggregationStrategy,
    /// Number of gradient pushes received since last aggregate.
    push_count: usize,
}

impl ParameterServer {
    /// Creates a new parameter server.
    pub fn new(worker_count: usize, strategy: AggregationStrategy) -> Self {
        Self {
            weights: HashMap::new(),
            grad_accumulator: HashMap::new(),
            worker_count,
            barrier_arrived: vec![false; worker_count],
            strategy,
            push_count: 0,
        }
    }

    /// Initializes a named parameter with the given weights.
    pub fn init_weights(&mut self, name: &str, weights: Array2<f64>) {
        self.weights.insert(name.to_string(), weights);
    }

    /// Returns the current weight for a parameter name.
    pub fn get_weights(&self, name: &str) -> Option<&Array2<f64>> {
        self.weights.get(name)
    }

    /// Returns all current weights.
    pub fn pull_weights(&self) -> &HashMap<String, Array2<f64>> {
        &self.weights
    }

    /// Receives gradients from a worker for a named parameter.
    ///
    /// Accumulates gradients until all workers have pushed.
    pub fn push_gradients(&mut self, _worker_rank: usize, gradients: HashMap<String, Array2<f64>>) {
        for (name, grad) in gradients {
            self.grad_accumulator.entry(name).or_default().push(grad);
        }
        self.push_count += 1;
    }

    /// Returns whether all workers have pushed gradients.
    pub fn all_gradients_received(&self) -> bool {
        self.push_count >= self.worker_count
    }

    /// Aggregates accumulated gradients and updates weights.
    ///
    /// Uses the configured aggregation strategy.
    /// Returns `Ok(())` on success.
    pub fn aggregate_and_update(&mut self, learning_rate: f64) -> Result<(), TensorError> {
        for (name, grads) in &self.grad_accumulator {
            if grads.is_empty() {
                continue;
            }
            let aggregated = aggregate_gradients(grads, &self.strategy)?;
            if let Some(w) = self.weights.get_mut(name) {
                *w = &*w - &(&aggregated * learning_rate);
            }
        }
        self.grad_accumulator.clear();
        self.push_count = 0;
        Ok(())
    }

    /// Records a worker reaching the barrier point.
    ///
    /// Returns `true` if all workers have arrived.
    pub fn barrier(&mut self, rank: usize) -> bool {
        if rank < self.barrier_arrived.len() {
            self.barrier_arrived[rank] = true;
        }
        let all = self.barrier_arrived.iter().all(|&v| v);
        if all {
            // Reset barrier for next round
            for v in &mut self.barrier_arrived {
                *v = false;
            }
        }
        all
    }

    /// Returns the number of workers.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Returns the push count since last aggregate.
    pub fn pending_push_count(&self) -> usize {
        self.push_count
    }
}

/// Aggregates a list of gradient arrays using the specified strategy.
fn aggregate_gradients(
    grads: &[Array2<f64>],
    strategy: &AggregationStrategy,
) -> Result<Array2<f64>, TensorError> {
    if grads.is_empty() {
        return Err(TensorError::InvalidData {
            reason: "no gradients to aggregate".to_string(),
        });
    }

    let summed = sum_arrays(grads)?;

    match strategy {
        AggregationStrategy::AllReduceMean => {
            let n = grads.len() as f64;
            Ok(summed / n)
        }
        AggregationStrategy::AllReduceSum => Ok(summed),
        AggregationStrategy::TopK(k) => {
            let mean = &summed / grads.len() as f64;
            Ok(apply_topk_sparsity(&mean, *k))
        }
    }
}

/// Sums a list of 2D arrays element-wise.
fn sum_arrays(arrays: &[Array2<f64>]) -> Result<Array2<f64>, TensorError> {
    let mut result = arrays[0].clone();
    for arr in &arrays[1..] {
        if result.shape() != arr.shape() {
            return Err(TensorError::ShapeMismatch {
                expected: result.shape().to_vec(),
                got: arr.shape().to_vec(),
            });
        }
        result = &result + arr;
    }
    Ok(result)
}

/// Applies TopK sparsity: keeps only top K% of gradients by magnitude, zeros rest.
fn apply_topk_sparsity(grad: &Array2<f64>, k_pct: f64) -> Array2<f64> {
    let flat: Vec<f64> = grad.iter().copied().collect();
    let total = flat.len();
    let keep_count = ((total as f64) * k_pct.clamp(0.0, 1.0)).ceil() as usize;

    if keep_count >= total {
        return grad.clone();
    }

    // Find the threshold magnitude
    let mut magnitudes: Vec<f64> = flat.iter().map(|v| v.abs()).collect();
    magnitudes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let threshold = if keep_count > 0 {
        magnitudes[keep_count - 1]
    } else {
        f64::INFINITY
    };

    // Keep values >= threshold, zero the rest
    grad.mapv(|v| if v.abs() >= threshold { v } else { 0.0 })
}

// ═══════════════════════════════════════════════════════════════════════
// Worker
// ═══════════════════════════════════════════════════════════════════════

/// A training worker that computes gradients on a data shard.
#[derive(Debug, Clone)]
pub struct Worker {
    /// Worker rank (ID).
    pub rank: usize,
    /// Local data shard (rows of training data).
    pub local_data: Array2<f64>,
    /// Local copy of model weights, keyed by name.
    pub weights: HashMap<String, Array2<f64>>,
    /// Current training step.
    pub step: usize,
}

impl Worker {
    /// Creates a new worker with the given rank and data shard.
    pub fn new(rank: usize, local_data: Array2<f64>) -> Self {
        Self {
            rank,
            local_data,
            weights: HashMap::new(),
            step: 0,
        }
    }

    /// Sets the local copy of weights from the parameter server.
    pub fn set_weights(&mut self, weights: HashMap<String, Array2<f64>>) {
        self.weights = weights;
    }

    /// Computes gradients on a mini-batch from local data.
    ///
    /// Simplified gradient computation: gradient = X^T * (X @ W - Y)
    /// where X is the input batch and Y is the target.
    ///
    /// Returns gradients keyed by parameter name.
    pub fn compute_gradients(
        &self,
        batch_start: usize,
        batch_size: usize,
    ) -> Result<HashMap<String, Array2<f64>>, TensorError> {
        let mut grads = HashMap::new();

        for (name, w) in &self.weights {
            let grad = compute_batch_gradient(&self.local_data, w, batch_start, batch_size);
            grads.insert(name.clone(), grad);
        }

        Ok(grads)
    }

    /// Performs a push-pull cycle with the parameter server.
    ///
    /// 1. Compute gradients on current batch
    /// 2. Push gradients to server
    /// 3. Pull updated weights from server
    pub fn push_and_pull(
        &mut self,
        server: &mut ParameterServer,
        batch_start: usize,
        batch_size: usize,
    ) -> Result<(), TensorError> {
        let grads = self.compute_gradients(batch_start, batch_size)?;
        server.push_gradients(self.rank, grads);
        self.step += 1;
        Ok(())
    }

    /// Updates local weights from the server.
    pub fn pull_from_server(&mut self, server: &ParameterServer) {
        self.weights = server.pull_weights().clone();
    }
}

/// Computes a simplified gradient for a batch of data.
///
/// Uses a linear model assumption: grad = X_batch^T * X_batch * W / batch_size
/// This is a simulation — real gradient would depend on loss function.
fn compute_batch_gradient(
    data: &Array2<f64>,
    weights: &Array2<f64>,
    batch_start: usize,
    batch_size: usize,
) -> Array2<f64> {
    let total_rows = data.nrows();
    let actual_start = batch_start.min(total_rows);
    let actual_end = (batch_start + batch_size).min(total_rows);

    if actual_start >= actual_end || data.ncols() == 0 {
        return Array2::zeros(weights.dim());
    }

    let batch = data
        .slice(ndarray::s![actual_start..actual_end, ..])
        .to_owned();
    let n = batch.nrows() as f64;

    // Simulated gradient: X^T @ X @ W / n (gradient of ||XW||^2)
    let xtx = batch.t().dot(&batch);
    if xtx.ncols() == weights.nrows() {
        xtx.dot(weights) / n
    } else {
        Array2::zeros(weights.dim())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Data Sharding
// ═══════════════════════════════════════════════════════════════════════

/// Shards data across workers by evenly splitting rows.
///
/// - `data`: full training data `[n_samples, features]`
/// - `rank`: worker rank (0-indexed)
/// - `world_size`: total number of workers
///
/// Returns the data shard for this worker.
pub fn shard_data(
    data: &Array2<f64>,
    rank: usize,
    world_size: usize,
) -> Result<Array2<f64>, TensorError> {
    if world_size == 0 {
        return Err(TensorError::InvalidData {
            reason: "world_size must be > 0".to_string(),
        });
    }
    if rank >= world_size {
        return Err(TensorError::InvalidData {
            reason: format!("rank {rank} >= world_size {world_size}"),
        });
    }

    let n = data.nrows();
    let shard_size = n / world_size;
    let start = rank * shard_size;
    let end = if rank == world_size - 1 {
        n // Last worker gets remainder
    } else {
        start + shard_size
    };

    if start >= n {
        return Ok(Array2::zeros((0, data.ncols())));
    }

    Ok(data.slice(ndarray::s![start..end, ..]).to_owned())
}

// ═══════════════════════════════════════════════════════════════════════
// Distributed Trainer (Orchestrator)
// ═══════════════════════════════════════════════════════════════════════

/// Orchestrates distributed training across server and workers.
///
/// Supports both synchronous and asynchronous training modes.
#[derive(Debug)]
pub struct DistributedTrainer {
    /// Parameter server.
    pub server: ParameterServer,
    /// Workers.
    pub workers: Vec<Worker>,
    /// Training mode.
    pub mode: TrainingMode,
    /// Learning rate.
    pub learning_rate: f64,
    /// Batch size per worker.
    pub batch_size: usize,
    /// Current global training step.
    pub global_step: usize,
}

impl DistributedTrainer {
    /// Creates a new distributed trainer.
    ///
    /// - `data`: full training data `[n_samples, features]`
    /// - `weights`: initial model weights, keyed by name
    /// - `n_workers`: number of workers
    /// - `strategy`: gradient aggregation strategy
    /// - `mode`: synchronous or asynchronous
    /// - `learning_rate`: optimizer learning rate
    /// - `batch_size`: per-worker batch size
    pub fn new(
        data: &Array2<f64>,
        weights: HashMap<String, Array2<f64>>,
        n_workers: usize,
        strategy: AggregationStrategy,
        mode: TrainingMode,
        learning_rate: f64,
        batch_size: usize,
    ) -> Result<Self, TensorError> {
        let mut server = ParameterServer::new(n_workers, strategy);
        for (name, w) in &weights {
            server.init_weights(name, w.clone());
        }

        let mut workers = Vec::with_capacity(n_workers);
        for rank in 0..n_workers {
            let shard = shard_data(data, rank, n_workers)?;
            let mut worker = Worker::new(rank, shard);
            worker.set_weights(weights.clone());
            workers.push(worker);
        }

        Ok(Self {
            server,
            workers,
            mode,
            learning_rate,
            batch_size,
            global_step: 0,
        })
    }

    /// Runs one training step (one round of gradient compute + update).
    ///
    /// In synchronous mode: all workers compute and push, barrier, aggregate, pull.
    /// In asynchronous mode: workers push independently, server aggregates per push.
    pub fn step(&mut self) -> Result<(), TensorError> {
        match &self.mode {
            TrainingMode::Synchronous => self.sync_step(),
            TrainingMode::Asynchronous { .. } => self.async_step(),
        }
    }

    /// Synchronous training step.
    fn sync_step(&mut self) -> Result<(), TensorError> {
        let batch_start = self.global_step * self.batch_size;

        // All workers compute and push gradients
        for i in 0..self.workers.len() {
            let grads = self.workers[i].compute_gradients(batch_start, self.batch_size)?;
            self.server.push_gradients(self.workers[i].rank, grads);
            self.server.barrier(self.workers[i].rank);
        }

        // Server aggregates and updates
        self.server.aggregate_and_update(self.learning_rate)?;

        // All workers pull updated weights
        for worker in &mut self.workers {
            worker.pull_from_server(&self.server);
            worker.step += 1;
        }

        self.global_step += 1;
        Ok(())
    }

    /// Asynchronous training step.
    fn async_step(&mut self) -> Result<(), TensorError> {
        let batch_start = self.global_step * self.batch_size;

        // Each worker independently pushes and pulls
        for i in 0..self.workers.len() {
            let grads = self.workers[i].compute_gradients(batch_start, self.batch_size)?;
            self.server.push_gradients(self.workers[i].rank, grads);

            // Server updates after each worker's push
            if self.server.pending_push_count() > 0 {
                self.server.aggregate_and_update(self.learning_rate)?;
            }

            self.workers[i].pull_from_server(&self.server);
            self.workers[i].step += 1;
        }

        self.global_step += 1;
        Ok(())
    }

    /// Runs multiple training steps.
    pub fn train(&mut self, num_steps: usize) -> Result<(), TensorError> {
        for _ in 0..num_steps {
            self.step()?;
        }
        Ok(())
    }

    /// Returns the current server weights.
    pub fn current_weights(&self) -> &HashMap<String, Array2<f64>> {
        self.server.pull_weights()
    }

    /// Returns the global training step count.
    pub fn global_step(&self) -> usize {
        self.global_step
    }

    /// Returns the number of workers.
    pub fn num_workers(&self) -> usize {
        self.workers.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> Array2<f64> {
        Array2::from_shape_vec((8, 4), (0..32).map(|i| i as f64 * 0.1).collect()).unwrap()
    }

    fn sample_weights() -> HashMap<String, Array2<f64>> {
        let mut w = HashMap::new();
        w.insert(
            "layer1".to_string(),
            Array2::from_shape_vec((4, 2), vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]).unwrap(),
        );
        w
    }

    // ── Sprint 16: Distributed Training ──

    #[test]
    fn s16_1_distributed_config() {
        let server = DistributedConfig::server(4);
        assert_eq!(server.role, Role::ParameterServer);
        assert_eq!(server.world_size, 4);
        assert_eq!(server.rank, 0);

        let worker = DistributedConfig::worker(2, 4);
        assert_eq!(worker.role, Role::Worker);
        assert_eq!(worker.rank, 2);
    }

    #[test]
    fn s16_2_shard_data_even_split() {
        let data = sample_data(); // 8 rows
        let shard0 = shard_data(&data, 0, 4).unwrap();
        let shard1 = shard_data(&data, 1, 4).unwrap();
        let shard2 = shard_data(&data, 2, 4).unwrap();
        let shard3 = shard_data(&data, 3, 4).unwrap();

        assert_eq!(shard0.nrows(), 2);
        assert_eq!(shard1.nrows(), 2);
        assert_eq!(shard2.nrows(), 2);
        assert_eq!(shard3.nrows(), 2);

        // Total rows should cover all data
        let total = shard0.nrows() + shard1.nrows() + shard2.nrows() + shard3.nrows();
        assert_eq!(total, 8);

        // Invalid rank should error
        assert!(shard_data(&data, 5, 4).is_err());
        assert!(shard_data(&data, 0, 0).is_err());
    }

    #[test]
    fn s16_3_parameter_server_init_and_pull() {
        let mut server = ParameterServer::new(2, AggregationStrategy::AllReduceMean);
        let w = Array2::ones((4, 2));
        server.init_weights("fc1", w.clone());

        assert!(server.get_weights("fc1").is_some());
        assert_eq!(server.get_weights("fc1").unwrap().shape(), &[4, 2]);
        assert!(server.get_weights("fc2").is_none());
        assert_eq!(server.worker_count(), 2);
    }

    #[test]
    fn s16_4_gradient_push_and_aggregate_mean() {
        let mut server = ParameterServer::new(2, AggregationStrategy::AllReduceMean);
        let init_w = Array2::ones((2, 2));
        server.init_weights("w", init_w);

        // Worker 0 pushes gradient = [[1, 2], [3, 4]]
        let mut g0 = HashMap::new();
        g0.insert(
            "w".to_string(),
            Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap(),
        );
        server.push_gradients(0, g0);
        assert!(!server.all_gradients_received());

        // Worker 1 pushes gradient = [[3, 4], [5, 6]]
        let mut g1 = HashMap::new();
        g1.insert(
            "w".to_string(),
            Array2::from_shape_vec((2, 2), vec![3.0, 4.0, 5.0, 6.0]).unwrap(),
        );
        server.push_gradients(1, g1);
        assert!(server.all_gradients_received());

        // Aggregate with lr=0.1
        // Mean grad = [[2, 3], [4, 5]]
        // New w = [[1,1],[1,1]] - 0.1 * [[2,3],[4,5]] = [[0.8,0.7],[0.6,0.5]]
        server.aggregate_and_update(0.1).unwrap();
        let w = server.get_weights("w").unwrap();
        assert!((w[[0, 0]] - 0.8).abs() < 1e-10);
        assert!((w[[0, 1]] - 0.7).abs() < 1e-10);
        assert!((w[[1, 0]] - 0.6).abs() < 1e-10);
        assert!((w[[1, 1]] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn s16_5_gradient_aggregate_sum() {
        let mut server = ParameterServer::new(2, AggregationStrategy::AllReduceSum);
        server.init_weights("w", Array2::zeros((2, 2)));

        let mut g0 = HashMap::new();
        g0.insert("w".to_string(), Array2::ones((2, 2)));
        server.push_gradients(0, g0);

        let mut g1 = HashMap::new();
        g1.insert("w".to_string(), Array2::ones((2, 2)));
        server.push_gradients(1, g1);

        // Sum grad = [[2, 2], [2, 2]], lr=1.0
        // w = [[0,0],[0,0]] - 1.0 * [[2,2],[2,2]] = [[-2,-2],[-2,-2]]
        server.aggregate_and_update(1.0).unwrap();
        let w = server.get_weights("w").unwrap();
        assert!((w[[0, 0]] - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn s16_6_topk_sparsity() {
        let grad = Array2::from_shape_vec(
            (2, 5),
            vec![1.0, 0.1, 5.0, 0.2, 3.0, 0.05, 4.0, 0.15, 2.0, 0.01],
        )
        .unwrap();
        let sparse = apply_topk_sparsity(&grad, 0.4); // Keep top 40% = 4 values

        // Top 4 by magnitude: 5.0, 4.0, 3.0, 2.0
        let nonzero_count = sparse.iter().filter(|&&v| v != 0.0).count();
        assert!(
            nonzero_count <= 5,
            "TopK(0.4) should keep ~4 values, got {nonzero_count}"
        );
        // The 5.0 should definitely be kept
        assert!(sparse[[0, 2]].abs() > 0.0);
    }

    #[test]
    fn s16_7_worker_compute_gradients() {
        let data = Array2::from_shape_vec(
            (4, 3),
            vec![
                1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
            ],
        )
        .unwrap();
        let mut worker = Worker::new(0, data);
        let mut weights = HashMap::new();
        weights.insert(
            "w".to_string(),
            Array2::from_shape_vec((3, 2), vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]).unwrap(),
        );
        worker.set_weights(weights);

        let grads = worker.compute_gradients(0, 2).unwrap();
        assert!(grads.contains_key("w"));
        assert_eq!(grads["w"].shape(), &[3, 2]);
        // Gradients should be non-zero
        assert!(grads["w"].iter().any(|&v| v.abs() > 1e-10));
    }

    #[test]
    fn s16_8_barrier_synchronization() {
        let mut server = ParameterServer::new(3, AggregationStrategy::AllReduceMean);

        // Workers arrive one by one
        assert!(!server.barrier(0));
        assert!(!server.barrier(1));
        // Third worker completes the barrier
        assert!(server.barrier(2));

        // Barrier should be reset
        assert!(!server.barrier(0));
    }

    #[test]
    fn s16_9_distributed_trainer_sync_step() {
        let data = sample_data();
        let weights = sample_weights();

        let mut trainer = DistributedTrainer::new(
            &data,
            weights.clone(),
            2,
            AggregationStrategy::AllReduceMean,
            TrainingMode::Synchronous,
            0.01,
            2,
        )
        .unwrap();

        assert_eq!(trainer.num_workers(), 2);
        assert_eq!(trainer.global_step(), 0);

        // Run one step
        trainer.step().unwrap();
        assert_eq!(trainer.global_step(), 1);

        // Weights should have changed
        let new_weights = trainer.current_weights();
        let orig = &weights["layer1"];
        let updated = &new_weights["layer1"];
        // At least some weight should differ
        let any_changed = orig
            .iter()
            .zip(updated.iter())
            .any(|(&a, &b)| (a - b).abs() > 1e-15);
        assert!(any_changed, "weights should change after training step");
    }

    #[test]
    fn s16_10_distributed_trainer_async_multiple_steps() {
        let data = sample_data();
        let weights = sample_weights();

        let mut trainer = DistributedTrainer::new(
            &data,
            weights,
            3,
            AggregationStrategy::AllReduceMean,
            TrainingMode::Asynchronous { max_staleness: 2 },
            0.01,
            2,
        )
        .unwrap();

        // Run 3 steps
        trainer.train(3).unwrap();
        assert_eq!(trainer.global_step(), 3);

        // Each worker should have advanced
        for worker in &trainer.workers {
            assert_eq!(worker.step, 3);
        }
    }
}
