//! Optimizers — SGD and Adam parameter update strategies.
//!
//! Each optimizer takes a set of parameter tensors and updates them
//! using their accumulated gradients.

use ndarray::ArrayD;

use super::tensor::TensorValue;

// ═══════════════════════════════════════════════════════════════════════
// SGD
// ═══════════════════════════════════════════════════════════════════════

/// Stochastic Gradient Descent optimizer with optional momentum.
#[derive(Debug, Clone)]
pub struct SGD {
    /// Learning rate.
    lr: f64,
    /// Momentum factor (0.0 = no momentum).
    momentum: f64,
    /// Velocity buffers for momentum (one per parameter).
    velocities: Vec<Option<ArrayD<f64>>>,
}

impl SGD {
    /// Creates a new SGD optimizer.
    pub fn new(lr: f64, momentum: f64) -> Self {
        Self {
            lr,
            momentum,
            velocities: Vec::new(),
        }
    }

    /// Updates parameters using their gradients.
    ///
    /// `params[i].grad()` must be `Some` for the update to occur.
    pub fn step(&mut self, params: &mut [TensorValue]) {
        // Ensure velocity buffers match parameter count
        if self.velocities.len() < params.len() {
            self.velocities.resize(params.len(), None);
        }

        for (i, param) in params.iter_mut().enumerate() {
            if let Some(grad) = param.grad() {
                let grad = grad.clone();
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
}
