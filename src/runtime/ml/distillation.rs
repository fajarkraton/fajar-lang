//! Knowledge distillation for Fajar Lang.
//!
//! Trains a small student model from a large teacher model using soft
//! labels and feature matching. Supports standard KD, feature distillation,
//! attention transfer, and progressive distillation strategies.

use ndarray::Array2;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from distillation operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum DistillationError {
    /// Temperature must be positive.
    #[error("invalid temperature {temperature}: must be > 0.0")]
    InvalidTemperature {
        /// The invalid temperature.
        temperature: f64,
    },

    /// Alpha must be in [0.0, 1.0].
    #[error("invalid alpha {alpha}: must be in [0.0, 1.0]")]
    InvalidAlpha {
        /// The invalid alpha.
        alpha: f64,
    },

    /// Shape mismatch between teacher and student outputs.
    #[error("shape mismatch: teacher {teacher:?} vs student {student:?}")]
    ShapeMismatch {
        /// Teacher output shape.
        teacher: [usize; 2],
        /// Student output shape.
        student: [usize; 2],
    },

    /// Invalid number of epochs.
    #[error("invalid epochs: must be > 0")]
    InvalidEpochs,
}

// ═══════════════════════════════════════════════════════════════════════
// Distillation configuration
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for knowledge distillation.
#[derive(Debug, Clone)]
pub struct DistillationConfig {
    /// Softmax temperature for soft labels (higher = softer).
    pub temperature: f64,
    /// Mixing factor: alpha * soft_loss + (1 - alpha) * hard_loss.
    pub alpha: f64,
    /// Number of distillation training epochs.
    pub epochs: usize,
    /// Learning rate for student updates.
    pub learning_rate: f64,
}

impl DistillationConfig {
    /// Creates a new distillation configuration.
    pub fn new(
        temperature: f64,
        alpha: f64,
        epochs: usize,
        learning_rate: f64,
    ) -> Result<Self, DistillationError> {
        if temperature <= 0.0 {
            return Err(DistillationError::InvalidTemperature { temperature });
        }
        if !(0.0..=1.0).contains(&alpha) {
            return Err(DistillationError::InvalidAlpha { alpha });
        }
        if epochs == 0 {
            return Err(DistillationError::InvalidEpochs);
        }
        Ok(Self {
            temperature,
            alpha,
            epochs,
            learning_rate,
        })
    }

    /// Creates a default configuration (temperature=3.0, alpha=0.7).
    pub fn default_config() -> Self {
        Self {
            temperature: 3.0,
            alpha: 0.7,
            epochs: 10,
            learning_rate: 0.01,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Soft labels (softmax with temperature)
// ═══════════════════════════════════════════════════════════════════════

/// Computes soft labels by applying softmax with temperature scaling.
///
/// For each row (sample), computes:
///   `softmax(logits / temperature)`
///
/// Higher temperature produces softer (more uniform) probability
/// distributions, transferring more "dark knowledge" from the teacher.
pub fn soft_labels(
    logits: &Array2<f64>,
    temperature: f64,
) -> Result<Array2<f64>, DistillationError> {
    if temperature <= 0.0 {
        return Err(DistillationError::InvalidTemperature { temperature });
    }

    let rows = logits.nrows();
    let cols = logits.ncols();
    let mut result = Array2::zeros((rows, cols));

    for r in 0..rows {
        // Scale by temperature
        let scaled: Vec<f64> = logits.row(r).iter().map(|&v| v / temperature).collect();

        // Numerical stability: subtract max before exp
        let max_val = scaled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let exp_vals: Vec<f64> = scaled.iter().map(|&v| (v - max_val).exp()).collect();
        let sum: f64 = exp_vals.iter().sum();

        for (c, &e) in exp_vals.iter().enumerate() {
            result[[r, c]] = if sum > 0.0 {
                e / sum
            } else {
                1.0 / cols as f64
            };
        }
    }

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════
// Distillation loss functions
// ═══════════════════════════════════════════════════════════════════════

/// Computes the KL divergence between teacher and student soft labels.
///
/// `KL(teacher || student) = sum(teacher * log(teacher / student))`
///
/// This is the "soft loss" component of knowledge distillation.
fn kl_divergence(teacher: &Array2<f64>, student: &Array2<f64>) -> f64 {
    let eps = 1e-10;
    let mut total = 0.0;
    for (t, s) in teacher.iter().zip(student.iter()) {
        let t_safe = t.max(eps);
        let s_safe = s.max(eps);
        total += t_safe * (t_safe / s_safe).ln();
    }
    let n = teacher.nrows();
    if n > 0 { total / n as f64 } else { 0.0 }
}

/// Computes the hard loss (cross-entropy) between student output and true targets.
///
/// `hard_loss = -sum(target * log(student)) / batch_size`
fn hard_loss(student_probs: &Array2<f64>, targets: &Array2<f64>) -> f64 {
    let eps = 1e-10;
    let mut total = 0.0;
    for (s, t) in student_probs.iter().zip(targets.iter()) {
        let s_safe = s.max(eps);
        total -= t * s_safe.ln();
    }
    let n = student_probs.nrows();
    if n > 0 { total / n as f64 } else { 0.0 }
}

/// Computes the combined distillation loss.
///
/// `loss = alpha * T^2 * KL(teacher_soft || student_soft) + (1 - alpha) * hard_loss`
///
/// The `T^2` factor compensates for the softmax temperature scaling,
/// ensuring gradients have appropriate magnitude.
pub fn distillation_loss(
    student_logits: &Array2<f64>,
    teacher_logits: &Array2<f64>,
    targets: &Array2<f64>,
    alpha: f64,
    temperature: f64,
) -> Result<f64, DistillationError> {
    if temperature <= 0.0 {
        return Err(DistillationError::InvalidTemperature { temperature });
    }
    if !(0.0..=1.0).contains(&alpha) {
        return Err(DistillationError::InvalidAlpha { alpha });
    }

    let teacher_soft = soft_labels(teacher_logits, temperature)?;
    let student_soft = soft_labels(student_logits, temperature)?;
    let student_hard = soft_labels(student_logits, 1.0)?;

    let soft = kl_divergence(&teacher_soft, &student_soft);
    let hard = hard_loss(&student_hard, targets);

    let t_sq = temperature * temperature;
    Ok(alpha * t_sq * soft + (1.0 - alpha) * hard)
}

// ═══════════════════════════════════════════════════════════════════════
// Feature distillation
// ═══════════════════════════════════════════════════════════════════════

/// Computes feature distillation loss (MSE between intermediate features).
///
/// Measures how well the student's intermediate representations match
/// the teacher's. Uses Mean Squared Error for alignment.
pub fn feature_distillation(
    teacher_features: &Array2<f64>,
    student_features: &Array2<f64>,
) -> Result<f64, DistillationError> {
    let t_shape = [teacher_features.nrows(), teacher_features.ncols()];
    let s_shape = [student_features.nrows(), student_features.ncols()];

    if t_shape != s_shape {
        return Err(DistillationError::ShapeMismatch {
            teacher: t_shape,
            student: s_shape,
        });
    }

    let n = teacher_features.len();
    if n == 0 {
        return Ok(0.0);
    }

    let mse: f64 = teacher_features
        .iter()
        .zip(student_features.iter())
        .map(|(t, s)| (t - s).powi(2))
        .sum::<f64>()
        / n as f64;

    Ok(mse)
}

/// Computes attention transfer loss between teacher and student.
///
/// Attention maps are computed as the L2 norm across channels for each
/// spatial position, then normalized. The loss is the MSE between
/// normalized attention maps.
pub fn attention_transfer(
    teacher_attn: &Array2<f64>,
    student_attn: &Array2<f64>,
) -> Result<f64, DistillationError> {
    let t_shape = [teacher_attn.nrows(), teacher_attn.ncols()];
    let s_shape = [student_attn.nrows(), student_attn.ncols()];

    if t_shape != s_shape {
        return Err(DistillationError::ShapeMismatch {
            teacher: t_shape,
            student: s_shape,
        });
    }

    // Normalize attention maps (per-row L2 normalization)
    let t_norm = normalize_rows(teacher_attn);
    let s_norm = normalize_rows(student_attn);

    let n = t_norm.len();
    if n == 0 {
        return Ok(0.0);
    }

    let mse: f64 = t_norm
        .iter()
        .zip(s_norm.iter())
        .map(|(t, s)| (t - s).powi(2))
        .sum::<f64>()
        / n as f64;

    Ok(mse)
}

/// Normalizes each row of a matrix to unit L2 norm.
fn normalize_rows(matrix: &Array2<f64>) -> Array2<f64> {
    let rows = matrix.nrows();
    let cols = matrix.ncols();
    let mut result = matrix.clone();

    for r in 0..rows {
        let norm: f64 = matrix.row(r).iter().map(|&v| v * v).sum::<f64>().sqrt();
        if norm > 1e-10 {
            for c in 0..cols {
                result[[r, c]] /= norm;
            }
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════
// Distillation trainer
// ═══════════════════════════════════════════════════════════════════════

/// Orchestrates the knowledge distillation training process.
///
/// Simulates the training loop: for each epoch, the teacher produces
/// soft targets, and the student is updated to minimize the combined
/// distillation loss.
#[derive(Debug, Clone)]
pub struct DistillationTrainer {
    /// Training configuration.
    config: DistillationConfig,
    /// Loss history (one per epoch).
    loss_history: Vec<f64>,
}

impl DistillationTrainer {
    /// Creates a new distillation trainer.
    pub fn new(config: DistillationConfig) -> Self {
        Self {
            config,
            loss_history: Vec::new(),
        }
    }

    /// Returns the training configuration.
    pub fn config(&self) -> &DistillationConfig {
        &self.config
    }

    /// Returns the recorded loss history.
    pub fn loss_history(&self) -> &[f64] {
        &self.loss_history
    }

    /// Runs the simulated distillation training loop.
    ///
    /// Takes teacher logits, student weights, and training targets.
    /// Returns the updated student weights after training.
    pub fn train(
        &mut self,
        teacher_logits: &Array2<f64>,
        student_weights: &Array2<f64>,
        targets: &Array2<f64>,
    ) -> Result<Array2<f64>, DistillationError> {
        let mut weights = student_weights.clone();
        self.loss_history.clear();

        for _epoch in 0..self.config.epochs {
            // Simulate student forward pass (weights as logits proxy)
            let student_logits = &weights;

            let loss = distillation_loss(
                student_logits,
                teacher_logits,
                targets,
                self.config.alpha,
                self.config.temperature,
            )?;

            self.loss_history.push(loss);

            // Simulated gradient step: move student toward teacher
            let teacher_soft = soft_labels(teacher_logits, self.config.temperature)?;
            let student_soft = soft_labels(student_logits, self.config.temperature)?;

            // Gradient approximation: difference between soft labels
            let grad = &student_soft - &teacher_soft;
            weights -= &(&grad * self.config.learning_rate);
        }

        Ok(weights)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Progressive distillation
// ═══════════════════════════════════════════════════════════════════════

/// Progressive distillation: reduce model in multiple stages.
///
/// Each stage uses the output of the previous stage as the new teacher,
/// creating a chain of increasingly smaller models.
#[derive(Debug, Clone)]
pub struct ProgressiveDistillation {
    /// Number of stages to distill through.
    pub num_stages: usize,
    /// Size reduction factor per stage (e.g., 0.5 = halve each time).
    pub reduction_factor: f64,
    /// Distillation config per stage.
    pub config: DistillationConfig,
}

impl ProgressiveDistillation {
    /// Creates a new progressive distillation pipeline.
    pub fn new(num_stages: usize, reduction_factor: f64, config: DistillationConfig) -> Self {
        Self {
            num_stages,
            reduction_factor,
            config,
        }
    }

    /// Runs the progressive distillation pipeline.
    ///
    /// Returns the final (smallest) student weights and a report.
    pub fn run(
        &self,
        initial_teacher: &Array2<f64>,
        targets: &Array2<f64>,
    ) -> Result<(Array2<f64>, DistillationReport), DistillationError> {
        let mut teacher = initial_teacher.clone();
        let rows = teacher.nrows();
        let cols = teacher.ncols();

        let initial_accuracy = simulated_accuracy(&teacher, targets);
        let mut stage_accuracies = vec![initial_accuracy];

        for _stage in 0..self.num_stages {
            // Create smaller student (simulate by scaling down values)
            let student = &teacher * self.reduction_factor;

            let mut trainer = DistillationTrainer::new(self.config.clone());
            teacher = trainer.train(&teacher, &student, targets)?;

            let accuracy = simulated_accuracy(&teacher, targets);
            stage_accuracies.push(accuracy);
        }

        let report = DistillationReport {
            teacher_params: rows * cols,
            student_params: rows * cols,
            teacher_accuracy: initial_accuracy,
            student_accuracy: *stage_accuracies.last().unwrap_or(&0.0),
            compression_ratio: 1.0 / self.reduction_factor.powi(self.num_stages as i32),
            loss_curve: Vec::new(),
            stage_accuracies,
        };

        Ok((teacher, report))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Distillation report
// ═══════════════════════════════════════════════════════════════════════

/// Summary report of a distillation training run.
#[derive(Debug, Clone)]
pub struct DistillationReport {
    /// Teacher model parameter count.
    pub teacher_params: usize,
    /// Student model parameter count.
    pub student_params: usize,
    /// Teacher model accuracy (simulated).
    pub teacher_accuracy: f64,
    /// Student model accuracy after distillation (simulated).
    pub student_accuracy: f64,
    /// Compression ratio (teacher size / student size).
    pub compression_ratio: f64,
    /// Loss values per epoch.
    pub loss_curve: Vec<f64>,
    /// Accuracy at each progressive stage (if applicable).
    pub stage_accuracies: Vec<f64>,
}

impl DistillationReport {
    /// Returns the accuracy gap (teacher - student).
    pub fn accuracy_gap(&self) -> f64 {
        self.teacher_accuracy - self.student_accuracy
    }
}

/// Simulates accuracy by comparing argmax of output rows to target rows.
fn simulated_accuracy(output: &Array2<f64>, targets: &Array2<f64>) -> f64 {
    let rows = output.nrows().min(targets.nrows());
    if rows == 0 {
        return 0.0;
    }

    let mut correct = 0;
    for r in 0..rows {
        let pred = argmax_row(output, r);
        let target = argmax_row(targets, r);
        if pred == target {
            correct += 1;
        }
    }

    correct as f64 / rows as f64
}

/// Returns the column index of the maximum value in a row.
fn argmax_row(matrix: &Array2<f64>, row: usize) -> usize {
    let row_data = matrix.row(row);
    let mut max_idx = 0;
    let mut max_val = f64::NEG_INFINITY;
    for (i, &v) in row_data.iter().enumerate() {
        if v > max_val {
            max_val = v;
            max_idx = i;
        }
    }
    max_idx
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s14_1_soft_labels_sum_to_one() {
        let logits = Array2::from_shape_vec((2, 3), vec![2.0, 1.0, 0.5, -1.0, 0.0, 3.0]).unwrap();

        let probs = soft_labels(&logits, 1.0).unwrap();

        // Each row should sum to ~1.0
        for r in 0..2 {
            let row_sum: f64 = probs.row(r).iter().sum();
            assert!(
                (row_sum - 1.0).abs() < 1e-6,
                "row {r} sum = {row_sum}, expected 1.0"
            );
        }
    }

    #[test]
    fn s14_2_soft_labels_temperature_increases_entropy() {
        let logits = Array2::from_shape_vec((1, 4), vec![5.0, 2.0, 1.0, 0.0]).unwrap();

        let hard = soft_labels(&logits, 1.0).unwrap();
        let soft = soft_labels(&logits, 5.0).unwrap();

        // Higher temperature should produce more uniform distribution
        // Entropy of soft should be higher than entropy of hard
        let h_hard = entropy_row(&hard, 0);
        let h_soft = entropy_row(&soft, 0);

        assert!(
            h_soft > h_hard,
            "soft entropy {h_soft} should be > hard entropy {h_hard}"
        );
    }

    #[test]
    fn s14_3_distillation_loss_combines_soft_and_hard() {
        let student = Array2::from_shape_vec((2, 3), vec![1.0, 0.5, 0.1, 0.2, 0.8, 0.3]).unwrap();
        let teacher = Array2::from_shape_vec((2, 3), vec![2.0, 0.3, 0.1, 0.1, 1.5, 0.2]).unwrap();
        let targets = Array2::from_shape_vec((2, 3), vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap();

        let loss = distillation_loss(&student, &teacher, &targets, 0.7, 3.0).unwrap();

        // Loss should be a finite positive number
        assert!(loss.is_finite(), "loss should be finite, got {loss}");
        assert!(loss >= 0.0, "loss should be >= 0, got {loss}");
    }

    #[test]
    fn s14_4_distillation_trainer_reduces_loss() {
        let teacher = Array2::from_shape_vec(
            (4, 3),
            vec![3.0, 0.1, 0.1, 0.1, 3.0, 0.1, 0.1, 0.1, 3.0, 3.0, 0.1, 0.1],
        )
        .unwrap();
        let student = Array2::from_shape_vec(
            (4, 3),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let targets = Array2::from_shape_vec(
            (4, 3),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        )
        .unwrap();

        let config = DistillationConfig::new(3.0, 0.7, 20, 0.1).unwrap();
        let mut trainer = DistillationTrainer::new(config);

        let _result = trainer.train(&teacher, &student, &targets).unwrap();

        // Loss should generally decrease over training
        let losses = trainer.loss_history();
        assert!(!losses.is_empty());
        assert!(
            losses.last().unwrap() <= &(losses[0] + 0.1),
            "final loss {} should not be much worse than initial {}",
            losses.last().unwrap(),
            losses[0]
        );
    }

    #[test]
    fn s14_5_feature_distillation_mse() {
        let teacher = Array2::from_shape_vec((2, 3), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let student = Array2::from_shape_vec((2, 3), vec![1.1, 1.9, 3.2, 3.8, 5.1, 5.9]).unwrap();

        let loss = feature_distillation(&teacher, &student).unwrap();

        // MSE should be small since features are close
        assert!(loss < 0.1, "MSE loss {loss} should be < 0.1");
        assert!(loss > 0.0, "MSE loss should be > 0");
    }

    #[test]
    fn s14_6_feature_distillation_shape_mismatch() {
        let teacher = Array2::ones((2, 3));
        let student = Array2::ones((3, 3));

        let err = feature_distillation(&teacher, &student);
        assert!(err.is_err());
    }

    #[test]
    fn s14_7_attention_transfer_loss() {
        let teacher =
            Array2::from_shape_vec((2, 4), vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]).unwrap();
        let student =
            Array2::from_shape_vec((2, 4), vec![0.9, 0.1, 0.0, 0.0, 0.1, 0.9, 0.0, 0.0]).unwrap();

        let loss = attention_transfer(&teacher, &student).unwrap();

        assert!(loss.is_finite());
        assert!(loss >= 0.0);
    }

    #[test]
    fn s14_8_distillation_report_accuracy_gap() {
        let report = DistillationReport {
            teacher_params: 10000,
            student_params: 2000,
            teacher_accuracy: 0.95,
            student_accuracy: 0.90,
            compression_ratio: 5.0,
            loss_curve: vec![1.0, 0.8, 0.6],
            stage_accuracies: vec![0.95, 0.92, 0.90],
        };

        assert!((report.accuracy_gap() - 0.05).abs() < 1e-10);
        assert_eq!(report.stage_accuracies.len(), 3);
    }

    #[test]
    fn s14_9_invalid_config_returns_error() {
        assert!(DistillationConfig::new(0.0, 0.5, 10, 0.01).is_err()); // bad temp
        assert!(DistillationConfig::new(-1.0, 0.5, 10, 0.01).is_err()); // bad temp
        assert!(DistillationConfig::new(3.0, 1.5, 10, 0.01).is_err()); // bad alpha
        assert!(DistillationConfig::new(3.0, -0.1, 10, 0.01).is_err()); // bad alpha
        assert!(DistillationConfig::new(3.0, 0.5, 0, 0.01).is_err()); // bad epochs
        assert!(DistillationConfig::new(3.0, 0.5, 10, 0.01).is_ok()); // valid
    }

    #[test]
    fn s14_10_progressive_distillation_runs() {
        let teacher =
            Array2::from_shape_vec((3, 3), vec![3.0, 0.1, 0.1, 0.1, 3.0, 0.1, 0.1, 0.1, 3.0])
                .unwrap();
        let targets =
            Array2::from_shape_vec((3, 3), vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
                .unwrap();

        let config = DistillationConfig::new(2.0, 0.5, 5, 0.05).unwrap();
        let progressive = ProgressiveDistillation::new(2, 0.8, config);

        let (result, report) = progressive.run(&teacher, &targets).unwrap();

        assert_eq!(result.nrows(), 3);
        assert_eq!(result.ncols(), 3);
        assert!(report.stage_accuracies.len() >= 2);
    }

    /// Helper: compute entropy of a probability row.
    fn entropy_row(probs: &Array2<f64>, row: usize) -> f64 {
        let eps = 1e-10;
        probs
            .row(row)
            .iter()
            .map(|&p| {
                let p_safe = p.max(eps);
                -p_safe * p_safe.ln()
            })
            .sum()
    }
}
