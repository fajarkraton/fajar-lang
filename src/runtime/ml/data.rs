//! Data loading utilities — CSV loading, batching, shuffling.
//!
//! Provides a `DataLoader` for iterating over datasets in batches,
//! with optional shuffling.

use super::tensor::TensorValue;

/// A simple dataset: features (X) and labels (y) as tensors.
#[derive(Debug, Clone)]
pub struct Dataset {
    /// Feature data: shape `[num_samples, num_features]`.
    pub x: Vec<Vec<f64>>,
    /// Label data: one value per sample.
    pub y: Vec<f64>,
}

/// An iterator over batches of a dataset.
pub struct DataLoader {
    /// Feature data.
    x: Vec<Vec<f64>>,
    /// Label data.
    y: Vec<f64>,
    /// Batch size.
    batch_size: usize,
    /// Current index.
    index: usize,
    /// Shuffled indices.
    indices: Vec<usize>,
}

impl Dataset {
    /// Creates a dataset from feature and label vectors.
    pub fn new(x: Vec<Vec<f64>>, y: Vec<f64>) -> Self {
        Self { x, y }
    }

    /// Parses a CSV string into a Dataset.
    ///
    /// Assumes the last column is the label. All values must be numeric.
    pub fn from_csv(csv: &str) -> Result<Self, String> {
        let mut x = Vec::new();
        let mut y = Vec::new();

        for (line_num, line) in csv.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let values: Result<Vec<f64>, _> =
                line.split(',').map(|s| s.trim().parse::<f64>()).collect();
            let values = values.map_err(|e| format!("line {}: {e}", line_num + 1))?;
            if values.is_empty() {
                continue;
            }
            let (features, label) = values.split_at(values.len() - 1);
            x.push(features.to_vec());
            y.push(label[0]);
        }

        if x.is_empty() {
            return Err("empty dataset".into());
        }

        Ok(Self { x, y })
    }

    /// Returns the number of samples.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Returns true if the dataset is empty.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Creates a DataLoader for this dataset.
    pub fn loader(&self, batch_size: usize, shuffle: bool) -> DataLoader {
        let n = self.x.len();
        let mut indices: Vec<usize> = (0..n).collect();
        if shuffle {
            fisher_yates_shuffle(&mut indices);
        }
        DataLoader {
            x: self.x.clone(),
            y: self.y.clone(),
            batch_size,
            index: 0,
            indices,
        }
    }
}

impl DataLoader {
    /// Returns the next batch as (features_tensor, labels_tensor).
    ///
    /// Features shape: `[batch_size, num_features]`
    /// Labels shape: `[batch_size, 1]`
    /// Returns `None` when all data has been consumed.
    pub fn next_batch(&mut self) -> Option<(TensorValue, TensorValue)> {
        if self.index >= self.indices.len() {
            return None;
        }

        let end = (self.index + self.batch_size).min(self.indices.len());
        let batch_indices = &self.indices[self.index..end];
        let actual_batch = batch_indices.len();
        self.index = end;

        if actual_batch == 0 {
            return None;
        }

        let num_features = self.x[0].len();
        let mut x_data = Vec::with_capacity(actual_batch * num_features);
        let mut y_data = Vec::with_capacity(actual_batch);

        for &idx in batch_indices {
            x_data.extend_from_slice(&self.x[idx]);
            y_data.push(self.y[idx]);
        }

        let x_tensor = TensorValue::from_data(x_data, &[actual_batch, num_features]).ok()?;
        let y_tensor = TensorValue::from_data(y_data, &[actual_batch, 1]).ok()?;

        Some((x_tensor, y_tensor))
    }

    /// Resets the iterator to the beginning, optionally re-shuffling.
    pub fn reset(&mut self, shuffle: bool) {
        self.index = 0;
        if shuffle {
            fisher_yates_shuffle(&mut self.indices);
        }
    }
}

/// Fisher-Yates shuffle (in-place).
fn fisher_yates_shuffle(arr: &mut [usize]) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    // Simple pseudo-random using system time as seed
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let mut state = hasher.finish();

    for i in (1..arr.len()).rev() {
        // Simple xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        arr.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_parse_basic() {
        let csv = "1.0,2.0,0\n3.0,4.0,1\n5.0,6.0,0";
        let ds = Dataset::from_csv(csv).unwrap();
        assert_eq!(ds.len(), 3);
        assert_eq!(ds.x[0], vec![1.0, 2.0]);
        assert_eq!(ds.y[0], 0.0);
        assert_eq!(ds.y[1], 1.0);
    }

    #[test]
    fn csv_parse_with_header_comment() {
        let csv = "# header\n1.0,0\n2.0,1";
        let ds = Dataset::from_csv(csv).unwrap();
        assert_eq!(ds.len(), 2);
    }

    #[test]
    fn csv_parse_empty_error() {
        let csv = "";
        assert!(Dataset::from_csv(csv).is_err());
    }

    #[test]
    fn dataloader_batches() {
        let ds = Dataset::new(
            vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0], vec![5.0]],
            vec![0.0, 1.0, 0.0, 1.0, 0.0],
        );
        let mut loader = ds.loader(2, false);

        let (x1, y1) = loader.next_batch().unwrap();
        assert_eq!(x1.shape(), &[2, 1]);
        assert_eq!(y1.shape(), &[2, 1]);

        let (x2, _y2) = loader.next_batch().unwrap();
        assert_eq!(x2.shape(), &[2, 1]);

        // Last batch: only 1 sample
        let (x3, _y3) = loader.next_batch().unwrap();
        assert_eq!(x3.shape(), &[1, 1]);

        // No more batches
        assert!(loader.next_batch().is_none());
    }

    #[test]
    fn dataloader_reset() {
        let ds = Dataset::new(vec![vec![1.0], vec![2.0]], vec![0.0, 1.0]);
        let mut loader = ds.loader(10, false);
        let _ = loader.next_batch();
        assert!(loader.next_batch().is_none());

        loader.reset(false);
        assert!(loader.next_batch().is_some());
    }

    #[test]
    fn shuffle_produces_permutation() {
        let mut indices: Vec<usize> = (0..100).collect();
        let original = indices.clone();
        fisher_yates_shuffle(&mut indices);
        // Shuffled should be a permutation (same elements, different order)
        let mut sorted = indices.clone();
        sorted.sort();
        assert_eq!(sorted, original);
        // Very unlikely to be identical after shuffle
        assert_ne!(indices, original);
    }
}
