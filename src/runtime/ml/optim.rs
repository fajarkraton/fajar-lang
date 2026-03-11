//! Optimizers — SGD, Adam, and AdamW parameter update strategies.
//!
//! Each optimizer takes a set of parameter tensors and updates them
//! using their accumulated gradients. Includes learning rate schedulers
//! and gradient clipping utilities.

use ndarray::ArrayD;

use super::tensor::TensorValue;

// ═══════════════════════════════════════════════════════════════════════
// SGD
// ═══════════════════════════════════════════════════════════════════════

/// Stochastic Gradient Descent optimizer with optional momentum and weight decay.
#[derive(Debug, Clone)]
pub struct SGD {
    /// Learning rate.
    lr: f64,
    /// Momentum factor (0.0 = no momentum).
    momentum: f64,
    /// L2 weight decay factor (0.0 = no weight decay).
    weight_decay: f64,
    /// Velocity buffers for momentum (one per parameter).
    velocities: Vec<Option<ArrayD<f64>>>,
}

impl SGD {
    /// Creates a new SGD optimizer.
    pub fn new(lr: f64, momentum: f64) -> Self {
        Self {
            lr,
            momentum,
            weight_decay: 0.0,
            velocities: Vec::new(),
        }
    }

    /// Creates a new SGD optimizer with weight decay.
    pub fn with_weight_decay(lr: f64, momentum: f64, weight_decay: f64) -> Self {
        Self {
            lr,
            momentum,
            weight_decay,
            velocities: Vec::new(),
        }
    }

    /// Updates parameters using their gradients.
    ///
    /// `params[i].grad()` must be `Some` for the update to occur.
    /// If weight_decay > 0, adds L2 penalty to gradients.
    pub fn step(&mut self, params: &mut [TensorValue]) {
        // Ensure velocity buffers match parameter count
        if self.velocities.len() < params.len() {
            self.velocities.resize(params.len(), None);
        }

        for (i, param) in params.iter_mut().enumerate() {
            if let Some(grad) = param.grad() {
                let mut grad = grad.clone();

                // L2 weight decay: add weight_decay * param to gradient
                if self.weight_decay != 0.0 {
                    grad += &(param.data() * self.weight_decay);
                }

                if self.momentum != 0.0 {
                    let velocity = match &self.velocities[i] {
                        Some(v) => v * self.momentum + &grad,
                        None => grad.clone(),
                    };
                    *param.data_mut() -= &(&velocity * self.lr);
                    self.velocities[i] = Some(velocity);
                } else {
                    *param.data_mut() -= &(&grad * self.lr);
                }
            }
        }
    }

    /// Returns the learning rate.
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Sets the learning rate.
    pub fn set_lr(&mut self, lr: f64) {
        self.lr = lr;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Adam
// ═══════════════════════════════════════════════════════════════════════

/// Adam optimizer (Adaptive Moment Estimation).
#[derive(Debug, Clone)]
pub struct Adam {
    /// Learning rate.
    lr: f64,
    /// Exponential decay rate for first moment estimates.
    beta1: f64,
    /// Exponential decay rate for second moment estimates.
    beta2: f64,
    /// Small constant for numerical stability.
    epsilon: f64,
    /// First moment (mean of gradients) for each parameter.
    m: Vec<Option<ArrayD<f64>>>,
    /// Second moment (mean of squared gradients) for each parameter.
    v: Vec<Option<ArrayD<f64>>>,
    /// Timestep counter.
    t: u64,
}

impl Adam {
    /// Creates a new Adam optimizer with default hyperparameters.
    pub fn new(lr: f64) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            m: Vec::new(),
            v: Vec::new(),
            t: 0,
        }
    }

    /// Creates a new Adam optimizer with custom hyperparameters.
    pub fn with_params(lr: f64, beta1: f64, beta2: f64, epsilon: f64) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            epsilon,
            m: Vec::new(),
            v: Vec::new(),
            t: 0,
        }
    }

    /// Updates parameters using their gradients.
    pub fn step(&mut self, params: &mut [TensorValue]) {
        self.t += 1;

        // Ensure buffers match parameter count
        if self.m.len() < params.len() {
            self.m.resize(params.len(), None);
            self.v.resize(params.len(), None);
        }

        let bias_correction1 = 1.0 - self.beta1.powi(self.t as i32);
        let bias_correction2 = 1.0 - self.beta2.powi(self.t as i32);

        for (i, param) in params.iter_mut().enumerate() {
            if let Some(grad) = param.grad() {
                let grad = grad.clone();

                // Update first moment: m = beta1 * m + (1 - beta1) * grad
                let m_new = match &self.m[i] {
                    Some(m_prev) => m_prev * self.beta1 + &grad * (1.0 - self.beta1),
                    None => &grad * (1.0 - self.beta1),
                };

                // Update second moment: v = beta2 * v + (1 - beta2) * grad^2
                let grad_sq = &grad * &grad;
                let v_new = match &self.v[i] {
                    Some(v_prev) => v_prev * self.beta2 + &grad_sq * (1.0 - self.beta2),
                    None => &grad_sq * (1.0 - self.beta2),
                };

                // Bias-corrected estimates
                let m_hat = &m_new / bias_correction1;
                let v_hat = &v_new / bias_correction2;

                // Update: param -= lr * m_hat / (sqrt(v_hat) + epsilon)
                let update = &m_hat / &(v_hat.mapv(f64::sqrt) + self.epsilon) * self.lr;
                *param.data_mut() -= &update;

                self.m[i] = Some(m_new);
                self.v[i] = Some(v_new);
            }
        }
    }

    /// Returns the learning rate.
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Sets the learning rate.
    pub fn set_lr(&mut self, lr: f64) {
        self.lr = lr;
    }

    /// Returns the current timestep.
    pub fn timestep(&self) -> u64 {
        self.t
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AdamW
// ═══════════════════════════════════════════════════════════════════════

/// AdamW optimizer — Adam with decoupled weight decay.
///
/// Unlike L2 regularization in standard Adam, AdamW applies weight decay
/// directly to the parameters, separate from the gradient update.
/// This leads to better generalization in practice.
#[derive(Debug, Clone)]
pub struct AdamW {
    /// Learning rate.
    lr: f64,
    /// Exponential decay rate for first moment estimates.
    beta1: f64,
    /// Exponential decay rate for second moment estimates.
    beta2: f64,
    /// Small constant for numerical stability.
    epsilon: f64,
    /// Decoupled weight decay factor.
    weight_decay: f64,
    /// First moment (mean of gradients) for each parameter.
    m: Vec<Option<ArrayD<f64>>>,
    /// Second moment (mean of squared gradients) for each parameter.
    v: Vec<Option<ArrayD<f64>>>,
    /// Timestep counter.
    t: u64,
}

impl AdamW {
    /// Creates a new AdamW optimizer with default hyperparameters.
    pub fn new(lr: f64, weight_decay: f64) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            weight_decay,
            m: Vec::new(),
            v: Vec::new(),
            t: 0,
        }
    }

    /// Creates a new AdamW optimizer with custom hyperparameters.
    pub fn with_params(lr: f64, beta1: f64, beta2: f64, epsilon: f64, weight_decay: f64) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            epsilon,
            weight_decay,
            m: Vec::new(),
            v: Vec::new(),
            t: 0,
        }
    }

    /// Updates parameters using their gradients with decoupled weight decay.
    ///
    /// The update rule is:
    /// 1. `m = beta1 * m + (1 - beta1) * grad`
    /// 2. `v = beta2 * v + (1 - beta2) * grad^2`
    /// 3. `param -= lr * (m_hat / (sqrt(v_hat) + eps) + weight_decay * param)`
    pub fn step(&mut self, params: &mut [TensorValue]) {
        self.t += 1;

        // Ensure buffers match parameter count
        if self.m.len() < params.len() {
            self.m.resize(params.len(), None);
            self.v.resize(params.len(), None);
        }

        let bias_correction1 = 1.0 - self.beta1.powi(self.t as i32);
        let bias_correction2 = 1.0 - self.beta2.powi(self.t as i32);

        for (i, param) in params.iter_mut().enumerate() {
            if let Some(grad) = param.grad() {
                let grad = grad.clone();

                // Update first moment: m = beta1 * m + (1 - beta1) * grad
                let m_new = match &self.m[i] {
                    Some(m_prev) => m_prev * self.beta1 + &grad * (1.0 - self.beta1),
                    None => &grad * (1.0 - self.beta1),
                };

                // Update second moment: v = beta2 * v + (1 - beta2) * grad^2
                let grad_sq = &grad * &grad;
                let v_new = match &self.v[i] {
                    Some(v_prev) => v_prev * self.beta2 + &grad_sq * (1.0 - self.beta2),
                    None => &grad_sq * (1.0 - self.beta2),
                };

                // Bias-corrected estimates
                let m_hat = &m_new / bias_correction1;
                let v_hat = &v_new / bias_correction2;

                // Adam update
                let adam_update = &m_hat / &(v_hat.mapv(f64::sqrt) + self.epsilon) * self.lr;

                // Decoupled weight decay: separate from gradient
                let decay = param.data() * (self.lr * self.weight_decay);

                *param.data_mut() -= &(&adam_update + &decay);

                self.m[i] = Some(m_new);
                self.v[i] = Some(v_new);
            }
        }
    }

    /// Returns the learning rate.
    pub fn lr(&self) -> f64 {
        self.lr
    }

    /// Sets the learning rate.
    pub fn set_lr(&mut self, lr: f64) {
        self.lr = lr;
    }

    /// Returns the weight decay factor.
    pub fn weight_decay(&self) -> f64 {
        self.weight_decay
    }

    /// Returns the current timestep.
    pub fn timestep(&self) -> u64 {
        self.t
    }
}

/// Resets gradients for all parameters.
pub fn zero_grad(params: &mut [TensorValue]) {
    for param in params.iter_mut() {
        param.zero_grad();
    }
}

/// Clips gradients by value: clamps each gradient element to [-max_val, max_val].
pub fn clip_grad_value(params: &mut [TensorValue], max_val: f64) {
    for param in params.iter_mut() {
        if let Some(grad) = param.grad().cloned() {
            let clipped = grad.mapv(|v| v.clamp(-max_val, max_val));
            param.set_grad(clipped);
        }
    }
}

/// Clips gradients by norm: if total L2 norm exceeds `max_norm`, scale down.
///
/// Returns the total norm before clipping.
pub fn clip_grad_norm(params: &mut [TensorValue], max_norm: f64) -> f64 {
    let mut total_norm_sq = 0.0;
    for param in params.iter() {
        if let Some(grad) = param.grad() {
            total_norm_sq += grad.iter().map(|v| v * v).sum::<f64>();
        }
    }
    let total_norm = total_norm_sq.sqrt();

    if total_norm > max_norm {
        let scale = max_norm / total_norm;
        for param in params.iter_mut() {
            if let Some(grad) = param.grad().cloned() {
                param.set_grad(grad.mapv(|v| v * scale));
            }
        }
    }

    total_norm
}

// ═══════════════════════════════════════════════════════════════════════
// Learning Rate Schedulers
// ═══════════════════════════════════════════════════════════════════════

/// Learning rate scheduler strategy.
#[derive(Debug, Clone)]
pub enum LrScheduler {
    /// Step decay: multiply LR by `gamma` every `step_size` epochs.
    Step {
        /// Initial learning rate.
        base_lr: f64,
        /// Multiply factor per step.
        gamma: f64,
        /// Epochs between each LR drop.
        step_size: u64,
    },
    /// Exponential decay: LR = base_lr * gamma^epoch.
    Exponential {
        /// Initial learning rate.
        base_lr: f64,
        /// Decay factor per epoch.
        gamma: f64,
    },
    /// Cosine annealing: LR oscillates between base_lr and min_lr.
    Cosine {
        /// Initial (maximum) learning rate.
        base_lr: f64,
        /// Minimum learning rate.
        min_lr: f64,
        /// Total number of epochs for one cycle.
        t_max: u64,
    },
}

impl LrScheduler {
    /// Computes the learning rate for a given epoch.
    pub fn get_lr(&self, epoch: u64) -> f64 {
        match self {
            LrScheduler::Step {
                base_lr,
                gamma,
                step_size,
            } => {
                let num_decays = epoch / step_size;
                base_lr * gamma.powi(num_decays as i32)
            }
            LrScheduler::Exponential { base_lr, gamma } => base_lr * gamma.powi(epoch as i32),
            LrScheduler::Cosine {
                base_lr,
                min_lr,
                t_max,
            } => {
                if *t_max == 0 {
                    return *base_lr;
                }
                let progress = std::f64::consts::PI * (epoch as f64) / (*t_max as f64);
                min_lr + (base_lr - min_lr) * (1.0 + progress.cos()) / 2.0
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Linear Warmup
// ═══════════════════════════════════════════════════════════════════════

/// Applies linear learning rate warmup over `warmup_steps` steps.
///
/// Returns the warmed-up learning rate for the given step.
/// After warmup, returns `base_lr`.
pub fn warmup_lr(step: u64, warmup_steps: u64, base_lr: f64) -> f64 {
    if warmup_steps == 0 || step >= warmup_steps {
        return base_lr;
    }
    base_lr * (step as f64) / (warmup_steps as f64)
}

// ═══════════════════════════════════════════════════════════════════════
// ReduceOnPlateau
// ═══════════════════════════════════════════════════════════════════════

/// Reduces learning rate when a metric has stopped improving.
///
/// Monitors a metric and reduces the LR by `factor` when no improvement
/// is seen for `patience` epochs.
#[derive(Debug, Clone)]
pub struct ReduceOnPlateau {
    /// Factor by which to reduce LR. new_lr = old_lr * factor.
    factor: f64,
    /// Number of epochs with no improvement after which LR will be reduced.
    patience: usize,
    /// Minimum learning rate.
    min_lr: f64,
    /// Whether lower metric is better (true for loss).
    minimize: bool,
    /// Minimum change to qualify as improvement.
    threshold: f64,
    /// Best metric value seen so far.
    best_metric: Option<f64>,
    /// Number of epochs with no improvement.
    counter: usize,
    /// Current learning rate.
    current_lr: f64,
}

impl ReduceOnPlateau {
    /// Creates a new ReduceOnPlateau scheduler.
    ///
    /// - `initial_lr`: starting learning rate
    /// - `factor`: multiply LR by this when reducing (e.g., 0.1)
    /// - `patience`: epochs to wait before reducing
    /// - `min_lr`: floor for learning rate
    /// - `minimize`: true if lower metric is better
    pub fn new(initial_lr: f64, factor: f64, patience: usize, min_lr: f64, minimize: bool) -> Self {
        Self {
            factor,
            patience,
            min_lr,
            minimize,
            threshold: 1e-4,
            best_metric: None,
            counter: 0,
            current_lr: initial_lr,
        }
    }

    /// Sets the improvement threshold.
    pub fn set_threshold(&mut self, threshold: f64) {
        self.threshold = threshold;
    }

    /// Updates the scheduler with the current metric value.
    ///
    /// Returns the (possibly reduced) learning rate.
    pub fn step_with_metric(&mut self, metric: f64) -> f64 {
        let improved = match self.best_metric {
            None => true,
            Some(best) => {
                if self.minimize {
                    metric < best - self.threshold
                } else {
                    metric > best + self.threshold
                }
            }
        };

        if improved {
            self.best_metric = Some(metric);
            self.counter = 0;
        } else {
            self.counter += 1;
            if self.counter >= self.patience {
                let new_lr = (self.current_lr * self.factor).max(self.min_lr);
                self.current_lr = new_lr;
                self.counter = 0;
            }
        }

        self.current_lr
    }

    /// Returns the current learning rate.
    pub fn get_lr(&self) -> f64 {
        self.current_lr
    }

    /// Returns the best metric value seen so far.
    pub fn best_metric(&self) -> Option<f64> {
        self.best_metric
    }
}

// ═══════════════════════════════════════════════════════════════════════
// OneCycleLR
// ═══════════════════════════════════════════════════════════════════════

/// One Cycle Learning Rate scheduler.
///
/// Three-phase schedule:
/// 1. **Warmup**: linearly ramp from `div_factor * max_lr` to `max_lr`
/// 2. **Decay**: cosine decay from `max_lr` to `min_lr`
/// 3. **Cooldown**: decay from `min_lr` to `final_div_factor * max_lr`
///
/// Phase boundaries are at `pct_start` of total steps.
#[derive(Debug, Clone)]
pub struct OneCycleLR {
    /// Maximum learning rate (peak).
    max_lr: f64,
    /// Total number of training steps.
    total_steps: u64,
    /// Fraction of total steps in warmup phase.
    pct_start: f64,
    /// Division factor for initial LR: initial_lr = max_lr / div_factor.
    div_factor: f64,
    /// Division factor for final LR: final_lr = max_lr / final_div_factor.
    final_div_factor: f64,
}

impl OneCycleLR {
    /// Creates a new OneCycleLR scheduler.
    ///
    /// - `max_lr`: peak learning rate
    /// - `total_steps`: total number of training steps
    /// - `pct_start`: fraction of steps for warmup (default 0.3)
    pub fn new(max_lr: f64, total_steps: u64) -> Self {
        Self {
            max_lr,
            total_steps,
            pct_start: 0.3,
            div_factor: 25.0,
            final_div_factor: 1e4,
        }
    }

    /// Creates a OneCycleLR scheduler with custom parameters.
    pub fn with_params(
        max_lr: f64,
        total_steps: u64,
        pct_start: f64,
        div_factor: f64,
        final_div_factor: f64,
    ) -> Self {
        Self {
            max_lr,
            total_steps,
            pct_start,
            div_factor,
            final_div_factor,
        }
    }

    /// Returns the learning rate at the given step.
    pub fn get_lr(&self, step: u64) -> f64 {
        if self.total_steps == 0 {
            return self.max_lr;
        }

        let initial_lr = self.max_lr / self.div_factor;
        let final_lr = self.max_lr / self.final_div_factor;
        let warmup_steps = (self.total_steps as f64 * self.pct_start) as u64;

        let step = step.min(self.total_steps);

        if step <= warmup_steps {
            // Phase 1: Linear warmup
            if warmup_steps == 0 {
                return self.max_lr;
            }
            let pct = step as f64 / warmup_steps as f64;
            initial_lr + (self.max_lr - initial_lr) * pct
        } else {
            // Phase 2: Cosine decay
            let decay_steps = self.total_steps - warmup_steps;
            if decay_steps == 0 {
                return final_lr;
            }
            let pct = (step - warmup_steps) as f64 / decay_steps as f64;
            let cos_val = (std::f64::consts::PI * pct).cos();
            final_lr + (self.max_lr - final_lr) * (1.0 + cos_val) / 2.0
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CosineAnnealingWarmRestarts
// ═══════════════════════════════════════════════════════════════════════

/// Cosine annealing with warm restarts (SGDR).
///
/// The LR follows a cosine curve from `base_lr` to `min_lr` over `t_0` epochs,
/// then resets. Each subsequent cycle is `t_mult` times longer.
#[derive(Debug, Clone)]
pub struct CosineAnnealingWarmRestarts {
    /// Base (maximum) learning rate.
    base_lr: f64,
    /// Minimum learning rate.
    min_lr: f64,
    /// Number of epochs for the first cycle.
    t_0: u64,
    /// Multiplicative factor for cycle length: next cycle = t_0 * t_mult^n.
    t_mult: f64,
}

impl CosineAnnealingWarmRestarts {
    /// Creates a new SGDR scheduler.
    ///
    /// - `base_lr`: maximum learning rate at each restart
    /// - `min_lr`: minimum learning rate
    /// - `t_0`: first cycle length (in epochs)
    /// - `t_mult`: cycle length multiplier (1.0 = fixed cycle, 2.0 = doubling)
    pub fn new(base_lr: f64, min_lr: f64, t_0: u64, t_mult: f64) -> Self {
        Self {
            base_lr,
            min_lr,
            t_0,
            t_mult,
        }
    }

    /// Returns the learning rate at the given epoch.
    pub fn get_lr(&self, epoch: u64) -> f64 {
        if self.t_0 == 0 {
            return self.base_lr;
        }

        // Find which cycle we're in and position within it
        let (cycle_pos, cycle_len) = self.find_cycle(epoch);

        // Cosine decay within the cycle
        let progress = std::f64::consts::PI * cycle_pos / cycle_len;
        self.min_lr + (self.base_lr - self.min_lr) * (1.0 + progress.cos()) / 2.0
    }

    /// Finds the position within the current cycle and the cycle length.
    fn find_cycle(&self, epoch: u64) -> (f64, f64) {
        if self.t_mult == 1.0 {
            // Fixed cycle length
            let t_0 = self.t_0 as f64;
            let pos = (epoch as f64) % t_0;
            return (pos, t_0);
        }

        // Variable cycle length: t_0, t_0 * t_mult, t_0 * t_mult^2, ...
        let mut remaining = epoch as f64;
        let mut cycle_len = self.t_0 as f64;

        loop {
            if remaining < cycle_len {
                return (remaining, cycle_len);
            }
            remaining -= cycle_len;
            cycle_len *= self.t_mult;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_param(data: Vec<f64>, shape: &[usize], grad: Vec<f64>) -> TensorValue {
        let mut t = TensorValue::from_data(data, shape).unwrap();
        t.set_requires_grad(true);
        let grad_arr = ArrayD::from_shape_vec(shape.to_vec(), grad).unwrap();
        t.set_grad(grad_arr);
        t
    }

    // ── SGD ──

    #[test]
    fn sgd_basic_update() {
        let mut sgd = SGD::new(0.1, 0.0);
        let mut params = vec![make_param(vec![1.0, 2.0], &[2], vec![10.0, 20.0])];
        sgd.step(&mut params);
        let data = params[0].to_vec();
        // 1.0 - 0.1 * 10.0 = 0.0, 2.0 - 0.1 * 20.0 = 0.0
        assert!((data[0] - 0.0).abs() < 1e-10);
        assert!((data[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn sgd_with_momentum() {
        let mut sgd = SGD::new(0.1, 0.9);
        let mut params = vec![make_param(vec![1.0], &[1], vec![1.0])];

        // Step 1: velocity = 1.0, update = 1.0 * 0.1 = 0.1 → param = 0.9
        sgd.step(&mut params);
        assert!((params[0].to_vec()[0] - 0.9).abs() < 1e-10);

        // Step 2: velocity = 0.9 * 1.0 + 1.0 = 1.9, update = 1.9 * 0.1 = 0.19
        // param = 0.9 - 0.19 = 0.71
        let grad_arr = ArrayD::from_shape_vec(vec![1], vec![1.0]).unwrap();
        params[0].set_grad(grad_arr);
        sgd.step(&mut params);
        assert!((params[0].to_vec()[0] - 0.71).abs() < 1e-10);
    }

    #[test]
    fn sgd_no_grad_no_update() {
        let mut sgd = SGD::new(0.1, 0.0);
        let mut t = TensorValue::from_data(vec![5.0], &[1]).unwrap();
        t.set_requires_grad(true);
        // No gradient set
        let mut params = vec![t];
        sgd.step(&mut params);
        assert_eq!(params[0].to_vec(), vec![5.0]);
    }

    #[test]
    fn sgd_lr() {
        let sgd = SGD::new(0.01, 0.0);
        assert_eq!(sgd.lr(), 0.01);
    }

    // ── Adam ──

    #[test]
    fn adam_basic_update() {
        let mut adam = Adam::new(0.001);
        let mut params = vec![make_param(vec![1.0, 2.0], &[2], vec![0.5, 0.5])];
        adam.step(&mut params);

        // After one step, parameters should have decreased
        assert!(params[0].to_vec()[0] < 1.0);
        assert!(params[0].to_vec()[1] < 2.0);
        assert_eq!(adam.timestep(), 1);
    }

    #[test]
    fn adam_multiple_steps() {
        let mut adam = Adam::new(0.1);
        let mut params = vec![make_param(vec![5.0], &[1], vec![1.0])];

        for _ in 0..10 {
            let grad_arr = ArrayD::from_shape_vec(vec![1], vec![1.0]).unwrap();
            params[0].set_grad(grad_arr);
            adam.step(&mut params);
        }

        // Should have moved significantly toward zero
        assert!(params[0].to_vec()[0] < 5.0);
        assert_eq!(adam.timestep(), 10);
    }

    #[test]
    fn adam_custom_params() {
        let adam = Adam::with_params(0.01, 0.8, 0.99, 1e-6);
        assert_eq!(adam.lr(), 0.01);
    }

    // ── zero_grad ──

    #[test]
    fn zero_grad_clears_all() {
        let mut params = vec![
            make_param(vec![1.0], &[1], vec![10.0]),
            make_param(vec![2.0, 3.0], &[2], vec![20.0, 30.0]),
        ];
        assert!(params[0].grad().is_some());
        assert!(params[1].grad().is_some());
        zero_grad(&mut params);
        assert!(params[0].grad().is_none());
        assert!(params[1].grad().is_none());
    }

    // ── Gradient clipping ──

    #[test]
    fn clip_grad_value_clamps() {
        let mut params = vec![make_param(vec![1.0], &[1], vec![10.0])];
        clip_grad_value(&mut params, 5.0);
        let grad = params[0].grad().unwrap();
        assert_eq!(grad.iter().next().copied().unwrap(), 5.0);
    }

    #[test]
    fn clip_grad_value_negative() {
        let mut params = vec![make_param(vec![1.0], &[1], vec![-10.0])];
        clip_grad_value(&mut params, 3.0);
        let grad = params[0].grad().unwrap();
        assert_eq!(grad.iter().next().copied().unwrap(), -3.0);
    }

    #[test]
    fn clip_grad_norm_scales_down() {
        // grad = [3, 4], norm = 5, clip to 2.5 → scale = 0.5
        let mut params = vec![make_param(vec![1.0, 2.0], &[2], vec![3.0, 4.0])];
        let norm = clip_grad_norm(&mut params, 2.5);
        assert!((norm - 5.0).abs() < 1e-6);
        let grad: Vec<f64> = params[0].grad().unwrap().iter().copied().collect();
        assert!((grad[0] - 1.5).abs() < 1e-6);
        assert!((grad[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn clip_grad_norm_no_clip_when_below() {
        let mut params = vec![make_param(vec![1.0], &[1], vec![1.0])];
        let norm = clip_grad_norm(&mut params, 10.0);
        assert!((norm - 1.0).abs() < 1e-6);
        // Should not be modified
        assert_eq!(
            params[0].grad().unwrap().iter().next().copied().unwrap(),
            1.0
        );
    }

    // ── LR Schedulers ──

    #[test]
    fn step_scheduler_decays_at_step_size() {
        let sched = LrScheduler::Step {
            base_lr: 0.1,
            gamma: 0.1,
            step_size: 10,
        };
        assert!((sched.get_lr(0) - 0.1).abs() < 1e-10);
        assert!((sched.get_lr(9) - 0.1).abs() < 1e-10);
        assert!((sched.get_lr(10) - 0.01).abs() < 1e-10);
        assert!((sched.get_lr(20) - 0.001).abs() < 1e-10);
    }

    #[test]
    fn exponential_scheduler_decays() {
        let sched = LrScheduler::Exponential {
            base_lr: 1.0,
            gamma: 0.5,
        };
        assert!((sched.get_lr(0) - 1.0).abs() < 1e-10);
        assert!((sched.get_lr(1) - 0.5).abs() < 1e-10);
        assert!((sched.get_lr(2) - 0.25).abs() < 1e-10);
        assert!((sched.get_lr(3) - 0.125).abs() < 1e-10);
    }

    #[test]
    fn cosine_scheduler_oscillates() {
        let sched = LrScheduler::Cosine {
            base_lr: 0.1,
            min_lr: 0.001,
            t_max: 100,
        };
        // Epoch 0: max LR
        assert!((sched.get_lr(0) - 0.1).abs() < 1e-10);
        // Epoch t_max: min LR
        assert!((sched.get_lr(100) - 0.001).abs() < 1e-10);
        // Epoch t_max/2: midpoint ≈ (0.1 + 0.001) / 2
        let mid = sched.get_lr(50);
        assert!((mid - 0.0505).abs() < 1e-3);
        // LR should decrease monotonically from 0 to t_max
        assert!(sched.get_lr(25) > sched.get_lr(75));
    }

    #[test]
    fn cosine_scheduler_zero_t_max() {
        let sched = LrScheduler::Cosine {
            base_lr: 0.1,
            min_lr: 0.0,
            t_max: 0,
        };
        assert!((sched.get_lr(0) - 0.1).abs() < 1e-10);
    }

    // ── SGD set_lr ──

    #[test]
    fn sgd_set_lr() {
        let mut sgd = SGD::new(0.1, 0.0);
        sgd.set_lr(0.01);
        assert_eq!(sgd.lr(), 0.01);
    }

    // ── SGD weight decay ──

    #[test]
    fn sgd_weight_decay_shrinks_params() {
        let mut sgd = SGD::with_weight_decay(0.1, 0.0, 0.01);
        let mut params = vec![make_param(vec![10.0], &[1], vec![0.0])];
        // grad=0 but weight_decay > 0, so param should still shrink
        sgd.step(&mut params);
        // update = lr * (grad + weight_decay * param) = 0.1 * (0.0 + 0.01 * 10.0) = 0.01
        assert!((params[0].to_vec()[0] - 9.99).abs() < 1e-6);
    }

    // ── AdamW ──

    #[test]
    fn adamw_basic_update() {
        let mut adamw = AdamW::new(0.001, 0.01);
        let mut params = vec![make_param(vec![1.0, 2.0], &[2], vec![0.5, 0.5])];
        adamw.step(&mut params);

        // Parameters should decrease (adam update + weight decay)
        assert!(params[0].to_vec()[0] < 1.0);
        assert!(params[0].to_vec()[1] < 2.0);
        assert_eq!(adamw.timestep(), 1);
    }

    #[test]
    fn adamw_weight_decay_decoupled() {
        // With zero gradient, AdamW should still shrink params via weight decay
        let mut adamw = AdamW::new(0.1, 0.01);
        let mut params = vec![make_param(vec![10.0], &[1], vec![0.0])];
        // grad = 0: Adam update is 0, but weight decay still applies
        // The Adam part with 0 gradient will produce a non-zero update due to
        // m_hat calculation, but weight decay additionally shrinks
        adamw.step(&mut params);
        assert!(params[0].to_vec()[0] < 10.0);
    }

    #[test]
    fn adamw_set_lr() {
        let mut adamw = AdamW::new(0.001, 0.01);
        adamw.set_lr(0.01);
        assert_eq!(adamw.lr(), 0.01);
    }

    #[test]
    fn adamw_weight_decay_accessor() {
        let adamw = AdamW::new(0.001, 0.05);
        assert_eq!(adamw.weight_decay(), 0.05);
    }

    #[test]
    fn adamw_custom_params() {
        let adamw = AdamW::with_params(0.01, 0.8, 0.99, 1e-6, 0.1);
        assert_eq!(adamw.lr(), 0.01);
        assert_eq!(adamw.weight_decay(), 0.1);
    }

    // ── Adam set_lr ──

    #[test]
    fn adam_set_lr() {
        let mut adam = Adam::new(0.001);
        adam.set_lr(0.01);
        assert_eq!(adam.lr(), 0.01);
    }

    // ── Linear Warmup ──

    #[test]
    fn warmup_lr_ramps_linearly() {
        assert!((warmup_lr(0, 100, 0.1) - 0.0).abs() < 1e-10);
        assert!((warmup_lr(50, 100, 0.1) - 0.05).abs() < 1e-10);
        assert!((warmup_lr(100, 100, 0.1) - 0.1).abs() < 1e-10);
        assert!((warmup_lr(200, 100, 0.1) - 0.1).abs() < 1e-10); // past warmup
    }

    #[test]
    fn warmup_lr_zero_steps() {
        assert!((warmup_lr(0, 0, 0.1) - 0.1).abs() < 1e-10);
    }

    // ── ReduceOnPlateau ──

    #[test]
    fn reduce_on_plateau_reduces_after_patience() {
        let mut rop = ReduceOnPlateau::new(0.1, 0.5, 3, 1e-6, true);
        rop.step_with_metric(1.0); // baseline
        rop.step_with_metric(1.0); // no improvement
        rop.step_with_metric(1.0); // no improvement
        let lr = rop.step_with_metric(1.0); // patience exceeded → reduce
        assert!((lr - 0.05).abs() < 1e-10); // 0.1 * 0.5
    }

    #[test]
    fn reduce_on_plateau_no_reduce_when_improving() {
        let mut rop = ReduceOnPlateau::new(0.1, 0.5, 3, 1e-6, true);
        rop.step_with_metric(1.0);
        rop.step_with_metric(0.9); // improving
        rop.step_with_metric(0.8); // improving
        let lr = rop.step_with_metric(0.7); // improving
        assert!((lr - 0.1).abs() < 1e-10); // no change
    }

    #[test]
    fn reduce_on_plateau_min_lr_floor() {
        let mut rop = ReduceOnPlateau::new(0.01, 0.1, 1, 0.001, true);
        rop.step_with_metric(1.0);
        rop.step_with_metric(1.0); // reduce to 0.001
        let lr = rop.step_with_metric(1.0); // would reduce to 0.0001 but min_lr=0.001
        assert!(lr >= 0.001);
    }

    #[test]
    fn reduce_on_plateau_maximize_mode() {
        let mut rop = ReduceOnPlateau::new(0.1, 0.5, 2, 1e-6, false);
        rop.step_with_metric(0.9);
        rop.step_with_metric(0.85); // worse (lower in maximize)
        let lr = rop.step_with_metric(0.8); // patience exceeded
        assert!((lr - 0.05).abs() < 1e-10);
    }

    #[test]
    fn reduce_on_plateau_best_metric() {
        let mut rop = ReduceOnPlateau::new(0.1, 0.5, 3, 1e-6, true);
        rop.step_with_metric(1.0);
        rop.step_with_metric(0.5);
        assert_eq!(rop.best_metric(), Some(0.5));
    }

    // ── OneCycleLR ──

    #[test]
    fn one_cycle_starts_low() {
        let sched = OneCycleLR::new(0.1, 100);
        let lr0 = sched.get_lr(0);
        // initial_lr = max_lr / div_factor = 0.1 / 25 = 0.004
        assert!((lr0 - 0.004).abs() < 1e-6);
    }

    #[test]
    fn one_cycle_peaks_at_warmup_end() {
        let sched = OneCycleLR::new(0.1, 100);
        let warmup_end = 30; // 0.3 * 100 = 30
        let lr_peak = sched.get_lr(warmup_end);
        assert!((lr_peak - 0.1).abs() < 1e-6);
    }

    #[test]
    fn one_cycle_ends_low() {
        let sched = OneCycleLR::new(0.1, 100);
        let lr_end = sched.get_lr(100);
        // final_lr = max_lr / final_div_factor = 0.1 / 10000 = 0.00001
        assert!((lr_end - 0.00001).abs() < 1e-6);
    }

    #[test]
    fn one_cycle_monotone_warmup() {
        let sched = OneCycleLR::new(0.1, 100);
        // During warmup, LR should increase
        let lr5 = sched.get_lr(5);
        let lr10 = sched.get_lr(10);
        let lr20 = sched.get_lr(20);
        assert!(lr5 < lr10);
        assert!(lr10 < lr20);
    }

    #[test]
    fn one_cycle_custom_params() {
        let sched = OneCycleLR::with_params(0.01, 50, 0.2, 10.0, 100.0);
        let lr0 = sched.get_lr(0);
        // initial_lr = 0.01 / 10 = 0.001
        assert!((lr0 - 0.001).abs() < 1e-6);
    }

    // ── CosineAnnealingWarmRestarts ──

    #[test]
    fn cosine_warm_restarts_starts_at_base() {
        let sched = CosineAnnealingWarmRestarts::new(0.1, 0.001, 10, 1.0);
        let lr = sched.get_lr(0);
        assert!((lr - 0.1).abs() < 1e-6);
    }

    #[test]
    fn cosine_warm_restarts_decays_within_cycle() {
        let sched = CosineAnnealingWarmRestarts::new(0.1, 0.001, 10, 1.0);
        let lr_mid = sched.get_lr(5);
        // At midpoint of cycle, should be near midpoint of [min_lr, base_lr]
        assert!(lr_mid > 0.001);
        assert!(lr_mid < 0.1);
    }

    #[test]
    fn cosine_warm_restarts_restarts() {
        let sched = CosineAnnealingWarmRestarts::new(0.1, 0.001, 10, 1.0);
        // End of first cycle (epoch 9): near min_lr
        let lr_end = sched.get_lr(9);
        // Start of second cycle (epoch 10): back to base_lr
        let lr_restart = sched.get_lr(10);
        assert!(lr_restart > lr_end);
        assert!((lr_restart - 0.1).abs() < 1e-6);
    }

    #[test]
    fn cosine_warm_restarts_t_mult() {
        let sched = CosineAnnealingWarmRestarts::new(0.1, 0.001, 10, 2.0);
        // First cycle: 10 epochs (0-9)
        // Second cycle: 20 epochs (10-29)
        // At epoch 10, restart
        let lr10 = sched.get_lr(10);
        assert!((lr10 - 0.1).abs() < 1e-6);
        // At epoch 30, restart again (10 + 20 = 30)
        let lr30 = sched.get_lr(30);
        assert!((lr30 - 0.1).abs() < 1e-6);
    }

    #[test]
    fn cosine_warm_restarts_zero_t0() {
        let sched = CosineAnnealingWarmRestarts::new(0.1, 0.001, 0, 1.0);
        assert!((sched.get_lr(0) - 0.1).abs() < 1e-10);
    }
}
