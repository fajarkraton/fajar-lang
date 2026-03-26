//! Data loading utilities — Dataset, DataLoader, EarlyStopping, Checkpoint.
//!
//! Provides ergonomic data loading, batching, shuffling, and training
//! utilities for ML workflows in Fajar Lang.

use std::collections::HashMap;

use ndarray::ArrayD;

use super::tensor::{TensorError, TensorValue};

// ═══════════════════════════════════════════════════════════════════════
// Dataset trait
// ═══════════════════════════════════════════════════════════════════════

/// A dataset provides indexed access to (feature, label) pairs.
pub trait Dataset {
    /// Returns the number of samples in the dataset.
    fn len(&self) -> usize;

    /// Returns whether the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the (features, labels) pair at the given index.
    ///
    /// Returns `None` if index is out of bounds.
    fn get(&self, index: usize) -> Option<(TensorValue, TensorValue)>;
}

// ═══════════════════════════════════════════════════════════════════════
// InMemoryDataset
// ═══════════════════════════════════════════════════════════════════════

/// An in-memory dataset storing all features and labels as tensors.
///
/// Features shape: `[num_samples, ...]`
/// Labels shape: `[num_samples, ...]`
#[derive(Debug, Clone)]
pub struct InMemoryDataset {
    /// Feature tensor `[num_samples, feature_dim...]`.
    features: TensorValue,
    /// Label tensor `[num_samples, label_dim...]`.
    labels: TensorValue,
    /// Number of samples.
    num_samples: usize,
}

impl InMemoryDataset {
    /// Creates a new in-memory dataset from feature and label tensors.
    ///
    /// The first dimension of both tensors must match (number of samples).
    pub fn new(features: TensorValue, labels: TensorValue) -> Result<Self, TensorError> {
        if features.shape().is_empty() || labels.shape().is_empty() {
            return Err(TensorError::InvalidData {
                reason: "features and labels must have at least 1 dimension".to_string(),
            });
        }
        let num_features = features.shape()[0];
        let num_labels = labels.shape()[0];
        if num_features != num_labels {
            return Err(TensorError::ShapeMismatch {
                expected: vec![num_features],
                got: vec![num_labels],
            });
        }
        Ok(Self {
            features,
            labels,
            num_samples: num_features,
        })
    }

    /// Returns a reference to the features tensor.
    pub fn features(&self) -> &TensorValue {
        &self.features
    }

    /// Returns a reference to the labels tensor.
    pub fn labels(&self) -> &TensorValue {
        &self.labels
    }
}

impl Dataset for InMemoryDataset {
    fn len(&self) -> usize {
        self.num_samples
    }

    fn get(&self, index: usize) -> Option<(TensorValue, TensorValue)> {
        if index >= self.num_samples {
            return None;
        }

        let feat_shape = self.features.shape();
        let label_shape = self.labels.shape();

        // Extract row `index` from features
        let feat_row = extract_row(self.features.data(), feat_shape, index);
        let label_row = extract_row(self.labels.data(), label_shape, index);

        // Shape for a single sample: remove the first dim
        let feat_sample_shape: Vec<usize> = feat_shape[1..].to_vec();
        let label_sample_shape: Vec<usize> = label_shape[1..].to_vec();

        let feat_shape_final = if feat_sample_shape.is_empty() {
            vec![1]
        } else {
            feat_sample_shape
        };
        let label_shape_final = if label_sample_shape.is_empty() {
            vec![1]
        } else {
            label_sample_shape
        };

        let f = TensorValue::from_data(feat_row, &feat_shape_final).ok()?;
        let l = TensorValue::from_data(label_row, &label_shape_final).ok()?;
        Some((f, l))
    }
}

/// Extracts a single row (first dimension index) from an ArrayD.
fn extract_row(data: &ArrayD<f64>, shape: &[usize], index: usize) -> Vec<f64> {
    let row_size: usize = shape[1..].iter().product::<usize>().max(1);
    let start = index * row_size;
    let flat: Vec<f64> = data.iter().copied().collect();
    flat[start..start + row_size].to_vec()
}

// ═══════════════════════════════════════════════════════════════════════
// Collate
// ═══════════════════════════════════════════════════════════════════════

/// Collates a batch of (features, labels) pairs into batch tensors.
///
/// Stacks individual samples along a new batch dimension (axis 0).
///
/// Returns `(batch_features, batch_labels)`.
pub fn collate_batch(
    samples: &[(TensorValue, TensorValue)],
) -> Result<(TensorValue, TensorValue), TensorError> {
    if samples.is_empty() {
        return Err(TensorError::InvalidData {
            reason: "cannot collate empty batch".to_string(),
        });
    }

    let batch_size = samples.len();
    let feat_shape = samples[0].0.shape().to_vec();
    let label_shape = samples[0].1.shape().to_vec();

    let mut feat_data = Vec::with_capacity(batch_size * samples[0].0.numel());
    let mut label_data = Vec::with_capacity(batch_size * samples[0].1.numel());

    for (f, l) in samples {
        if f.shape() != feat_shape.as_slice() {
            return Err(TensorError::ShapeMismatch {
                expected: feat_shape.clone(),
                got: f.shape().to_vec(),
            });
        }
        if l.shape() != label_shape.as_slice() {
            return Err(TensorError::ShapeMismatch {
                expected: label_shape.clone(),
                got: l.shape().to_vec(),
            });
        }
        feat_data.extend(f.to_vec());
        label_data.extend(l.to_vec());
    }

    let mut batch_feat_shape = vec![batch_size];
    batch_feat_shape.extend(&feat_shape);
    let mut batch_label_shape = vec![batch_size];
    batch_label_shape.extend(&label_shape);

    let batch_features = TensorValue::from_data(feat_data, &batch_feat_shape)?;
    let batch_labels = TensorValue::from_data(label_data, &batch_label_shape)?;

    Ok((batch_features, batch_labels))
}

// ═══════════════════════════════════════════════════════════════════════
// DataLoader
// ═══════════════════════════════════════════════════════════════════════

/// Loads data in batches from a dataset with optional shuffling.
#[derive(Debug)]
pub struct DataLoader<D: Dataset> {
    /// The underlying dataset.
    dataset: D,
    /// Number of samples per batch.
    batch_size: usize,
    /// Whether to shuffle indices each epoch.
    shuffle: bool,
    /// Random seed for reproducible shuffling.
    seed: u64,
}

impl<D: Dataset> DataLoader<D> {
    /// Creates a new DataLoader.
    pub fn new(dataset: D, batch_size: usize, shuffle: bool) -> Self {
        Self {
            dataset,
            batch_size,
            shuffle,
            seed: 42,
        }
    }

    /// Creates a DataLoader with a specific random seed.
    pub fn with_seed(dataset: D, batch_size: usize, shuffle: bool, seed: u64) -> Self {
        Self {
            dataset,
            batch_size,
            shuffle,
            seed,
        }
    }

    /// Sets the random seed for shuffling.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed;
    }

    /// Returns the number of batches per epoch.
    pub fn num_batches(&self) -> usize {
        let n = self.dataset.len();
        n.div_ceil(self.batch_size)
    }

    /// Returns a reference to the underlying dataset.
    pub fn dataset(&self) -> &D {
        &self.dataset
    }

    /// Returns the batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Creates an iterator over batches for one epoch.
    ///
    /// If shuffle is enabled, indices are shuffled using Fisher-Yates.
    /// Each call increments the internal seed for different shuffling per epoch.
    pub fn iter_epoch(&mut self) -> DataLoaderIter<'_, D> {
        let n = self.dataset.len();
        let mut indices: Vec<usize> = (0..n).collect();

        if self.shuffle {
            fisher_yates_shuffle(&mut indices, self.seed);
            self.seed = self.seed.wrapping_add(1);
        }

        DataLoaderIter {
            loader: self,
            indices,
            position: 0,
        }
    }
}

/// Iterator over batches from a DataLoader.
pub struct DataLoaderIter<'a, D: Dataset> {
    /// Reference to the parent DataLoader.
    loader: &'a DataLoader<D>,
    /// Shuffled index order for this epoch.
    indices: Vec<usize>,
    /// Current position in indices.
    position: usize,
}

impl<'a, D: Dataset> DataLoaderIter<'a, D> {
    /// Returns the next batch, or None if the epoch is complete.
    pub fn next_batch(&mut self) -> Option<Result<(TensorValue, TensorValue), TensorError>> {
        let n = self.indices.len();
        if self.position >= n {
            return None;
        }

        let end = (self.position + self.loader.batch_size).min(n);
        let batch_indices = &self.indices[self.position..end];
        self.position = end;

        let mut samples = Vec::with_capacity(batch_indices.len());
        for &idx in batch_indices {
            match self.loader.dataset.get(idx) {
                Some(sample) => samples.push(sample),
                None => {
                    return Some(Err(TensorError::InvalidData {
                        reason: format!("index {idx} out of bounds"),
                    }));
                }
            }
        }

        Some(collate_batch(&samples))
    }
}

/// Fisher-Yates shuffle with a simple LCG PRNG for deterministic shuffling.
fn fisher_yates_shuffle(indices: &mut [usize], seed: u64) {
    let n = indices.len();
    if n <= 1 {
        return;
    }
    let mut rng_state = seed;
    for i in (1..n).rev() {
        // Simple LCG: state = state * 6364136223846793005 + 1442695040888963407
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (rng_state >> 33) as usize % (i + 1);
        indices.swap(i, j);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EarlyStopping
// ═══════════════════════════════════════════════════════════════════════

/// Early stopping monitor: stops training when metric stops improving.
///
/// Tracks the best metric value and counts consecutive non-improving epochs.
/// When the counter exceeds `patience`, signals to stop training.
#[derive(Debug, Clone)]
pub struct EarlyStopping {
    /// Number of epochs to wait for improvement before stopping.
    patience: usize,
    /// Minimum change to qualify as improvement.
    min_delta: f64,
    /// Best metric value seen so far.
    best_metric: Option<f64>,
    /// Number of consecutive non-improving epochs.
    counter: usize,
    /// Whether lower is better (true for loss, false for accuracy).
    minimize: bool,
}

impl EarlyStopping {
    /// Creates a new EarlyStopping monitor.
    ///
    /// - `patience`: epochs to wait before stopping
    /// - `min_delta`: minimum improvement threshold
    /// - `minimize`: true if lower metric is better (e.g., loss)
    pub fn new(patience: usize, min_delta: f64, minimize: bool) -> Self {
        Self {
            patience,
            min_delta,
            best_metric: None,
            counter: 0,
            minimize,
        }
    }

    /// Updates with the current epoch's metric value.
    ///
    /// Returns `true` if training should stop (patience exceeded).
    pub fn step(&mut self, metric: f64) -> bool {
        let improved = match self.best_metric {
            None => true,
            Some(best) => {
                if self.minimize {
                    metric < best - self.min_delta
                } else {
                    metric > best + self.min_delta
                }
            }
        };

        if improved {
            self.best_metric = Some(metric);
            self.counter = 0;
        } else {
            self.counter += 1;
        }

        self.counter >= self.patience
    }

    /// Returns the best metric value seen so far.
    pub fn best_metric(&self) -> Option<f64> {
        self.best_metric
    }

    /// Returns the current patience counter.
    pub fn counter(&self) -> usize {
        self.counter
    }

    /// Resets the early stopping state.
    pub fn reset(&mut self) {
        self.best_metric = None;
        self.counter = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Checkpoint
// ═══════════════════════════════════════════════════════════════════════

/// A training checkpoint: captures model params, optimizer state, and metadata.
///
/// Serializable to/from JSON for persistence.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Epoch number when checkpoint was created.
    pub epoch: usize,
    /// Learning rate at checkpoint time.
    pub lr: f64,
    /// Best metric value at checkpoint time.
    pub best_metric: Option<f64>,
    /// Flattened parameter data: maps param name to flat `Vec<f64>`.
    pub params: HashMap<String, Vec<f64>>,
    /// Flattened optimizer state: maps state name to flat `Vec<f64>`.
    pub optimizer_state: HashMap<String, Vec<f64>>,
}

impl Checkpoint {
    /// Creates a new empty checkpoint.
    pub fn new(epoch: usize, lr: f64) -> Self {
        Self {
            epoch,
            lr,
            best_metric: None,
            params: HashMap::new(),
            optimizer_state: HashMap::new(),
        }
    }

    /// Adds a parameter snapshot to the checkpoint.
    pub fn add_param(&mut self, name: &str, tensor: &TensorValue) {
        self.params.insert(name.to_string(), tensor.to_vec());
    }

    /// Retrieves a parameter snapshot from the checkpoint.
    pub fn get_param(&self, name: &str) -> Option<&Vec<f64>> {
        self.params.get(name)
    }

    /// Adds optimizer state data to the checkpoint.
    pub fn add_optimizer_state(&mut self, name: &str, data: Vec<f64>) {
        self.optimizer_state.insert(name.to_string(), data);
    }

    /// Retrieves optimizer state data from the checkpoint.
    pub fn get_optimizer_state(&self, name: &str) -> Option<&Vec<f64>> {
        self.optimizer_state.get(name)
    }

    /// Serializes the checkpoint to a simple text format.
    ///
    /// Format: key=value lines, with param/state arrays as comma-separated floats.
    pub fn to_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("epoch={}", self.epoch));
        lines.push(format!("lr={}", self.lr));
        match self.best_metric {
            Some(m) => lines.push(format!("best_metric={m}")),
            None => lines.push("best_metric=none".to_string()),
        }
        for (name, data) in &self.params {
            let vals: Vec<String> = data.iter().map(|v| format!("{v}")).collect();
            lines.push(format!("param:{name}={}", vals.join(",")));
        }
        for (name, data) in &self.optimizer_state {
            let vals: Vec<String> = data.iter().map(|v| format!("{v}")).collect();
            lines.push(format!("optim:{name}={}", vals.join(",")));
        }
        lines.join("\n")
    }

    /// Deserializes a checkpoint from the text format.
    pub fn from_text(text: &str) -> Result<Self, TensorError> {
        let mut ckpt = Checkpoint::new(0, 0.0);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "epoch" => {
                        ckpt.epoch = value.parse().map_err(|_| TensorError::InvalidData {
                            reason: "invalid epoch".to_string(),
                        })?;
                    }
                    "lr" => {
                        ckpt.lr = value.parse().map_err(|_| TensorError::InvalidData {
                            reason: "invalid lr".to_string(),
                        })?;
                    }
                    "best_metric" => {
                        ckpt.best_metric = if value == "none" {
                            None
                        } else {
                            Some(value.parse().map_err(|_| TensorError::InvalidData {
                                reason: "invalid best_metric".to_string(),
                            })?)
                        };
                    }
                    _ if key.starts_with("param:") => {
                        let name = &key["param:".len()..];
                        let data = parse_float_list(value)?;
                        ckpt.params.insert(name.to_string(), data);
                    }
                    _ if key.starts_with("optim:") => {
                        let name = &key["optim:".len()..];
                        let data = parse_float_list(value)?;
                        ckpt.optimizer_state.insert(name.to_string(), data);
                    }
                    _ => {} // ignore unknown keys
                }
            }
        }

        Ok(ckpt)
    }
}

/// Parses a comma-separated list of floats.
fn parse_float_list(s: &str) -> Result<Vec<f64>, TensorError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .map_err(|_| TensorError::InvalidData {
                    reason: format!("invalid float in list: '{v}'"),
                })
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Training Mode
// ═══════════════════════════════════════════════════════════════════════

/// Training mode toggle for layers that behave differently during train/eval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainingMode {
    /// Training mode: enables dropout, uses batch statistics for batchnorm.
    Train,
    /// Evaluation mode: disables dropout, uses running statistics for batchnorm.
    Eval,
}

/// Tracks running statistics for BatchNorm during training.
///
/// Uses exponential moving average to accumulate mean and variance.
#[derive(Debug, Clone)]
pub struct RunningStats {
    /// Running mean: `[num_features]`.
    pub running_mean: ArrayD<f64>,
    /// Running variance: `[num_features]`.
    pub running_var: ArrayD<f64>,
    /// Momentum for EMA update.
    pub momentum: f64,
    /// Number of batches seen.
    pub num_batches: usize,
}

impl RunningStats {
    /// Creates new running statistics for `num_features` features.
    pub fn new(num_features: usize) -> Self {
        Self {
            running_mean: ArrayD::zeros(vec![num_features]),
            running_var: ArrayD::ones(vec![num_features]),
            momentum: 0.1,
            num_batches: 0,
        }
    }

    /// Creates running stats with a custom momentum value.
    pub fn with_momentum(num_features: usize, momentum: f64) -> Self {
        let mut s = Self::new(num_features);
        s.momentum = momentum;
        s
    }

    /// Updates running statistics with a new batch's mean and variance.
    pub fn update(&mut self, batch_mean: &ArrayD<f64>, batch_var: &ArrayD<f64>) {
        let m = self.momentum;
        self.running_mean = &self.running_mean * (1.0 - m) + batch_mean * m;
        self.running_var = &self.running_var * (1.0 - m) + batch_var * m;
        self.num_batches += 1;
    }

    /// Returns the current running mean.
    pub fn mean(&self) -> &ArrayD<f64> {
        &self.running_mean
    }

    /// Returns the current running variance.
    pub fn variance(&self) -> &ArrayD<f64> {
        &self.running_var
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── InMemoryDataset ──

    #[test]
    fn in_memory_dataset_creation() {
        let features = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]).unwrap();
        let labels = TensorValue::from_data(vec![0.0, 1.0, 0.0], &[3, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        assert_eq!(ds.len(), 3);
        assert!(!ds.is_empty());
    }

    #[test]
    fn in_memory_dataset_get_sample() {
        let features = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]).unwrap();
        let labels = TensorValue::from_data(vec![10.0, 20.0, 30.0], &[3, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();

        let (f, l) = ds.get(0).unwrap();
        assert_eq!(f.to_vec(), vec![1.0, 2.0]);
        assert_eq!(l.to_vec(), vec![10.0]);

        let (f, l) = ds.get(2).unwrap();
        assert_eq!(f.to_vec(), vec![5.0, 6.0]);
        assert_eq!(l.to_vec(), vec![30.0]);
    }

    #[test]
    fn in_memory_dataset_out_of_bounds() {
        let features = TensorValue::from_data(vec![1.0, 2.0], &[2, 1]).unwrap();
        let labels = TensorValue::from_data(vec![0.0, 1.0], &[2, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        assert!(ds.get(2).is_none());
    }

    #[test]
    fn in_memory_dataset_mismatched_samples_rejected() {
        let features = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let labels = TensorValue::from_data(vec![0.0, 1.0, 2.0], &[3, 1]).unwrap();
        let result = InMemoryDataset::new(features, labels);
        assert!(result.is_err());
    }

    // ── Collate ──

    #[test]
    fn collate_batch_stacks_samples() {
        let samples = vec![
            (
                TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap(),
                TensorValue::from_data(vec![10.0], &[1]).unwrap(),
            ),
            (
                TensorValue::from_data(vec![3.0, 4.0], &[2]).unwrap(),
                TensorValue::from_data(vec![20.0], &[1]).unwrap(),
            ),
        ];

        let (bf, bl) = collate_batch(&samples).unwrap();
        assert_eq!(bf.shape(), &[2, 2]);
        assert_eq!(bl.shape(), &[2, 1]);
        assert_eq!(bf.to_vec(), vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(bl.to_vec(), vec![10.0, 20.0]);
    }

    #[test]
    fn collate_empty_batch_error() {
        let samples: Vec<(TensorValue, TensorValue)> = vec![];
        assert!(collate_batch(&samples).is_err());
    }

    // ── DataLoader ──

    #[test]
    fn dataloader_num_batches_correct() {
        let features = TensorValue::from_data(vec![0.0; 20], &[10, 2]).unwrap();
        let labels = TensorValue::from_data(vec![0.0; 10], &[10, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        let loader = DataLoader::new(ds, 3, false);
        // 10 samples, batch_size=3 -> ceil(10/3) = 4 batches
        assert_eq!(loader.num_batches(), 4);
    }

    #[test]
    fn dataloader_iterates_all_samples() {
        let features = TensorValue::from_data(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]).unwrap();
        let labels = TensorValue::from_data(vec![10.0, 20.0, 30.0], &[3, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        let mut loader = DataLoader::new(ds, 2, false);

        let mut iter = loader.iter_epoch();
        let batch1 = iter.next_batch().unwrap().unwrap();
        assert_eq!(batch1.0.shape(), &[2, 2]); // first batch: 2 samples
        let batch2 = iter.next_batch().unwrap().unwrap();
        assert_eq!(batch2.0.shape(), &[1, 2]); // second batch: 1 sample (remainder)
        assert!(iter.next_batch().is_none()); // no more
    }

    #[test]
    fn dataloader_shuffle_changes_order() {
        let data: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let features = TensorValue::from_data(data.clone(), &[10, 2]).unwrap();
        let labels = TensorValue::from_data(vec![0.0; 10], &[10, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        let mut loader = DataLoader::with_seed(ds, 10, true, 123);

        let mut iter = loader.iter_epoch();
        let batch = iter.next_batch().unwrap().unwrap();
        let shuffled = batch.0.to_vec();
        // With shuffle, data should differ from original order
        // (extremely unlikely to be identical with 10 samples)
        assert_ne!(shuffled, data);
    }

    #[test]
    fn dataloader_batch_size_accessor() {
        let features = TensorValue::from_data(vec![0.0; 4], &[2, 2]).unwrap();
        let labels = TensorValue::from_data(vec![0.0; 2], &[2, 1]).unwrap();
        let ds = InMemoryDataset::new(features, labels).unwrap();
        let loader = DataLoader::new(ds, 32, false);
        assert_eq!(loader.batch_size(), 32);
    }

    // ── EarlyStopping ──

    #[test]
    fn early_stopping_no_stop_when_improving() {
        let mut es = EarlyStopping::new(3, 0.01, true);
        assert!(!es.step(1.0)); // first value, improvement
        assert!(!es.step(0.9)); // better
        assert!(!es.step(0.8)); // better
        assert_eq!(es.counter(), 0);
    }

    #[test]
    fn early_stopping_stops_after_patience() {
        let mut es = EarlyStopping::new(3, 0.01, true);
        es.step(1.0); // baseline
        es.step(1.1); // worse, counter=1
        es.step(1.1); // worse, counter=2
        let should_stop = es.step(1.1); // worse, counter=3 >= patience=3
        assert!(should_stop);
        assert_eq!(es.counter(), 3);
    }

    #[test]
    fn early_stopping_resets_on_improvement() {
        let mut es = EarlyStopping::new(3, 0.01, true);
        es.step(1.0);
        es.step(1.1); // worse
        es.step(1.1); // worse
        es.step(0.5); // improvement! counter resets
        assert_eq!(es.counter(), 0);
        assert_eq!(es.best_metric(), Some(0.5));
    }

    #[test]
    fn early_stopping_maximize_mode() {
        let mut es = EarlyStopping::new(2, 0.0, false); // maximize
        es.step(0.5);
        es.step(0.6); // better (higher)
        es.step(0.55); // worse, counter=1
        let stop = es.step(0.55); // worse, counter=2
        assert!(stop);
    }

    #[test]
    fn early_stopping_min_delta() {
        let mut es = EarlyStopping::new(2, 0.1, true);
        es.step(1.0);
        // 0.95 < 1.0 but not by min_delta (0.1), so NOT improvement
        es.step(0.95);
        assert_eq!(es.counter(), 1);
        // 0.85 < 1.0 - 0.1 = 0.9, so IS improvement
        es.step(0.85);
        assert_eq!(es.counter(), 0);
    }

    #[test]
    fn early_stopping_reset() {
        let mut es = EarlyStopping::new(3, 0.0, true);
        es.step(1.0);
        es.step(1.1);
        es.reset();
        assert_eq!(es.counter(), 0);
        assert_eq!(es.best_metric(), None);
    }

    // ── Checkpoint ──

    #[test]
    fn checkpoint_add_and_get_params() {
        let mut ckpt = Checkpoint::new(5, 0.001);
        let tensor = TensorValue::from_data(vec![1.0, 2.0, 3.0], &[3]).unwrap();
        ckpt.add_param("weight", &tensor);
        assert_eq!(ckpt.get_param("weight"), Some(&vec![1.0, 2.0, 3.0]));
        assert_eq!(ckpt.get_param("bias"), None);
    }

    #[test]
    fn checkpoint_text_roundtrip() {
        let mut ckpt = Checkpoint::new(10, 0.01);
        ckpt.best_metric = Some(0.95);
        ckpt.add_param("w", &TensorValue::from_data(vec![1.0, 2.0], &[2]).unwrap());
        ckpt.add_optimizer_state("m", vec![0.1, 0.2]);

        let text = ckpt.to_text();
        let loaded = Checkpoint::from_text(&text).unwrap();

        assert_eq!(loaded.epoch, 10);
        assert_eq!(loaded.lr, 0.01);
        assert_eq!(loaded.best_metric, Some(0.95));
        assert_eq!(loaded.get_param("w"), Some(&vec![1.0, 2.0]));
        assert_eq!(loaded.get_optimizer_state("m"), Some(&vec![0.1, 0.2]));
    }

    #[test]
    fn checkpoint_optimizer_state() {
        let mut ckpt = Checkpoint::new(0, 0.1);
        ckpt.add_optimizer_state("velocity", vec![0.5, 0.6, 0.7]);
        assert_eq!(
            ckpt.get_optimizer_state("velocity"),
            Some(&vec![0.5, 0.6, 0.7])
        );
    }

    // ── TrainingMode ──

    #[test]
    fn training_mode_equality() {
        assert_eq!(TrainingMode::Train, TrainingMode::Train);
        assert_ne!(TrainingMode::Train, TrainingMode::Eval);
    }

    // ── RunningStats ──

    #[test]
    fn running_stats_initial_values() {
        let stats = RunningStats::new(4);
        assert_eq!(stats.running_mean.shape(), &[4]);
        assert_eq!(stats.running_var.shape(), &[4]);
        assert!(stats.mean().iter().all(|&v| v == 0.0));
        assert!(stats.variance().iter().all(|&v| v == 1.0));
        assert_eq!(stats.num_batches, 0);
    }

    #[test]
    fn running_stats_update_ema() {
        let mut stats = RunningStats::with_momentum(2, 0.1);
        let batch_mean = ArrayD::from_shape_vec(vec![2], vec![1.0, 2.0]).unwrap();
        let batch_var = ArrayD::from_shape_vec(vec![2], vec![0.5, 0.5]).unwrap();
        stats.update(&batch_mean, &batch_var);

        // mean = 0 * 0.9 + 1.0 * 0.1 = 0.1, 0 * 0.9 + 2.0 * 0.1 = 0.2
        let mean: Vec<f64> = stats.mean().iter().copied().collect();
        assert!((mean[0] - 0.1).abs() < 1e-10);
        assert!((mean[1] - 0.2).abs() < 1e-10);

        // var = 1.0 * 0.9 + 0.5 * 0.1 = 0.95
        let var: Vec<f64> = stats.variance().iter().copied().collect();
        assert!((var[0] - 0.95).abs() < 1e-10);
        assert_eq!(stats.num_batches, 1);
    }
}
