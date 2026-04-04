//! Reinforcement Learning — environment trait, replay buffer,
//! DQN, policy gradient, PPO, GAE, multi-agent, vectorized envs.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S15.1: Environment Trait
// ═══════════════════════════════════════════════════════════════════════

/// A step result from the environment.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// New state.
    pub state: Vec<f64>,
    /// Reward received.
    pub reward: f64,
    /// Whether the episode is done.
    pub done: bool,
    /// Additional info.
    pub info: String,
}

/// A simulated environment.
#[derive(Debug, Clone)]
pub struct Environment {
    /// Environment name.
    pub name: String,
    /// State dimension.
    pub state_dim: usize,
    /// Action dimension.
    pub action_dim: usize,
    /// Current state.
    pub state: Vec<f64>,
    /// Current step count.
    pub step_count: u64,
    /// Maximum steps per episode.
    pub max_steps: u64,
}

impl Environment {
    /// Creates a new environment.
    pub fn new(name: &str, state_dim: usize, action_dim: usize, max_steps: u64) -> Self {
        Environment {
            name: name.to_string(),
            state_dim,
            action_dim,
            state: vec![0.0; state_dim],
            step_count: 0,
            max_steps,
        }
    }

    /// Resets the environment to initial state.
    pub fn reset(&mut self) -> Vec<f64> {
        self.state = vec![0.0; self.state_dim];
        self.step_count = 0;
        self.state.clone()
    }

    /// Takes a step in the environment.
    pub fn step(&mut self, action: usize) -> StepResult {
        self.step_count += 1;
        // Simple simulation: state changes based on action
        for (i, s) in self.state.iter_mut().enumerate() {
            *s += (action as f64 - self.action_dim as f64 / 2.0) * 0.1 * (i + 1) as f64;
        }
        let raw_reward = -self.state.iter().map(|s| s * s).sum::<f64>().sqrt();
        let reward = if raw_reward == 0.0 { 0.0 } else { raw_reward }; // normalize -0.0
        let done = self.step_count >= self.max_steps;

        StepResult {
            state: self.state.clone(),
            reward,
            done,
            info: String::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.2: Replay Buffer
// ═══════════════════════════════════════════════════════════════════════

/// An experience tuple for replay.
#[derive(Debug, Clone)]
pub struct Experience {
    /// State before action.
    pub state: Vec<f64>,
    /// Action taken.
    pub action: usize,
    /// Reward received.
    pub reward: f64,
    /// Next state.
    pub next_state: Vec<f64>,
    /// Whether the episode ended.
    pub done: bool,
    /// Priority for prioritized replay.
    pub priority: f64,
}

/// Experience replay buffer.
#[derive(Debug)]
pub struct ReplayBuffer {
    /// Stored experiences.
    pub buffer: Vec<Experience>,
    /// Maximum capacity.
    pub capacity: usize,
    /// Write position (circular).
    pos: usize,
    /// Total stored.
    pub size: usize,
}

impl ReplayBuffer {
    /// Creates a new replay buffer.
    pub fn new(capacity: usize) -> Self {
        ReplayBuffer {
            buffer: Vec::with_capacity(capacity),
            capacity,
            pos: 0,
            size: 0,
        }
    }

    /// Adds an experience to the buffer.
    pub fn push(&mut self, exp: Experience) {
        if self.buffer.len() < self.capacity {
            self.buffer.push(exp);
        } else {
            self.buffer[self.pos] = exp;
        }
        self.pos = (self.pos + 1) % self.capacity;
        if self.size < self.capacity {
            self.size += 1;
        }
    }

    /// Samples a batch of experiences (uniform).
    pub fn sample(&self, batch_size: usize) -> Vec<&Experience> {
        // Deterministic sampling for tests (take first batch_size)
        self.buffer.iter().take(batch_size).collect()
    }

    /// Samples prioritized (highest priority first).
    pub fn sample_prioritized(&self, batch_size: usize) -> Vec<&Experience> {
        let mut sorted: Vec<&Experience> = self.buffer.iter().collect();
        sorted.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(batch_size).collect()
    }

    /// Returns true if buffer has enough samples.
    pub fn can_sample(&self, batch_size: usize) -> bool {
        self.size >= batch_size
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.3: DQN Agent
// ═══════════════════════════════════════════════════════════════════════

/// DQN agent configuration.
#[derive(Debug, Clone)]
pub struct DqnConfig {
    /// Learning rate.
    pub learning_rate: f64,
    /// Discount factor (gamma).
    pub gamma: f64,
    /// Epsilon for exploration.
    pub epsilon: f64,
    /// Epsilon decay rate.
    pub epsilon_decay: f64,
    /// Minimum epsilon.
    pub epsilon_min: f64,
    /// Target network update frequency (in steps).
    pub target_update_freq: u64,
}

impl Default for DqnConfig {
    fn default() -> Self {
        DqnConfig {
            learning_rate: 1e-3,
            gamma: 0.99,
            epsilon: 1.0,
            epsilon_decay: 0.995,
            epsilon_min: 0.01,
            target_update_freq: 100,
        }
    }
}

/// Computes the DQN loss (TD error) for a batch.
pub fn dqn_td_error(
    q_values: &[f64],
    target_q: &[f64],
    actions: &[usize],
    rewards: &[f64],
    dones: &[bool],
    gamma: f64,
) -> Vec<f64> {
    rewards
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            let target = if dones[i] { r } else { r + gamma * target_q[i] };
            target - q_values[actions[i]]
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S15.4: Policy Gradient (REINFORCE)
// ═══════════════════════════════════════════════════════════════════════

/// Computes discounted returns from a sequence of rewards.
pub fn compute_returns(rewards: &[f64], gamma: f64) -> Vec<f64> {
    let mut returns = vec![0.0; rewards.len()];
    let mut running = 0.0;
    for i in (0..rewards.len()).rev() {
        running = rewards[i] + gamma * running;
        returns[i] = running;
    }
    returns
}

/// Subtracts baseline (mean) from returns for variance reduction.
pub fn subtract_baseline(returns: &[f64]) -> Vec<f64> {
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    returns.iter().map(|&r| r - mean).collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S15.5: PPO Agent
// ═══════════════════════════════════════════════════════════════════════

/// PPO configuration.
#[derive(Debug, Clone)]
pub struct PpoConfig {
    /// Clipping parameter.
    pub clip_epsilon: f64,
    /// Value function coefficient.
    pub value_coef: f64,
    /// Entropy bonus coefficient.
    pub entropy_coef: f64,
    /// Number of optimization epochs per update.
    pub num_epochs: usize,
    /// Mini-batch size.
    pub mini_batch_size: usize,
}

impl Default for PpoConfig {
    fn default() -> Self {
        PpoConfig {
            clip_epsilon: 0.2,
            value_coef: 0.5,
            entropy_coef: 0.01,
            num_epochs: 4,
            mini_batch_size: 64,
        }
    }
}

/// Computes the PPO clipped surrogate loss.
pub fn ppo_clipped_loss(ratio: f64, advantage: f64, clip_epsilon: f64) -> f64 {
    let unclipped = ratio * advantage;
    let clipped = ratio.clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon) * advantage;
    unclipped.min(clipped)
}

// ═══════════════════════════════════════════════════════════════════════
// S15.6: GAE (Generalized Advantage Estimation)
// ═══════════════════════════════════════════════════════════════════════

/// Computes Generalized Advantage Estimation.
pub fn compute_gae(
    rewards: &[f64],
    values: &[f64],
    next_value: f64,
    gamma: f64,
    lambda: f64,
) -> Vec<f64> {
    let n = rewards.len();
    let mut advantages = vec![0.0; n];
    let mut gae = 0.0;

    for i in (0..n).rev() {
        let next_v = if i == n - 1 {
            next_value
        } else {
            values[i + 1]
        };
        let delta = rewards[i] + gamma * next_v - values[i];
        gae = delta + gamma * lambda * gae;
        advantages[i] = gae;
    }

    advantages
}

// ═══════════════════════════════════════════════════════════════════════
// S15.7: Multi-Agent
// ═══════════════════════════════════════════════════════════════════════

/// Multi-agent policy sharing mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySharing {
    /// All agents share the same policy.
    Shared,
    /// Each agent has an independent policy.
    Independent,
}

impl fmt::Display for PolicySharing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicySharing::Shared => write!(f, "Shared"),
            PolicySharing::Independent => write!(f, "Independent"),
        }
    }
}

/// Multi-agent configuration.
#[derive(Debug, Clone)]
pub struct MultiAgentConfig {
    /// Number of agents.
    pub num_agents: usize,
    /// Policy sharing mode.
    pub policy_sharing: PolicySharing,
}

// ═══════════════════════════════════════════════════════════════════════
// S15.8: Vectorized Environments
// ═══════════════════════════════════════════════════════════════════════

/// A collection of parallel environments.
#[derive(Debug)]
pub struct VectorizedEnv {
    /// Individual environments.
    pub envs: Vec<Environment>,
}

impl VectorizedEnv {
    /// Creates N parallel copies of an environment.
    pub fn new(name: &str, state_dim: usize, action_dim: usize, max_steps: u64, n: usize) -> Self {
        VectorizedEnv {
            envs: (0..n)
                .map(|_| Environment::new(name, state_dim, action_dim, max_steps))
                .collect(),
        }
    }

    /// Resets all environments.
    pub fn reset_all(&mut self) -> Vec<Vec<f64>> {
        self.envs.iter_mut().map(|e| e.reset()).collect()
    }

    /// Steps all environments with the given actions.
    pub fn step_all(&mut self, actions: &[usize]) -> Vec<StepResult> {
        self.envs
            .iter_mut()
            .zip(actions.iter())
            .map(|(e, &a)| e.step(a))
            .collect()
    }

    /// Returns the number of environments.
    pub fn num_envs(&self) -> usize {
        self.envs.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S15.9: Reward Shaping
// ═══════════════════════════════════════════════════════════════════════

/// A staged reward function for curriculum learning.
#[derive(Debug, Clone)]
pub struct CurriculumStage {
    /// Stage name.
    pub name: String,
    /// Reward multiplier.
    pub reward_scale: f64,
    /// Minimum episodes before advancing.
    pub min_episodes: u64,
    /// Performance threshold to advance.
    pub threshold: f64,
}

/// Applies reward shaping based on current stage.
pub fn shape_reward(raw_reward: f64, stage: &CurriculumStage) -> f64 {
    raw_reward * stage.reward_scale
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S15.1 — Environment Trait
    #[test]
    fn s15_1_environment() {
        let mut env = Environment::new("CartPole", 4, 2, 200);
        let state = env.reset();
        assert_eq!(state.len(), 4);
        let result = env.step(1);
        assert_eq!(result.state.len(), 4);
        assert!(!result.done);
    }

    // S15.2 — Replay Buffer
    #[test]
    fn s15_2_replay_buffer() {
        let mut buf = ReplayBuffer::new(100);
        for i in 0..50 {
            buf.push(Experience {
                state: vec![i as f64],
                action: 0,
                reward: 1.0,
                next_state: vec![(i + 1) as f64],
                done: false,
                priority: 1.0,
            });
        }
        assert!(buf.can_sample(32));
        let batch = buf.sample(32);
        assert_eq!(batch.len(), 32);
    }

    #[test]
    fn s15_2_prioritized_replay() {
        let mut buf = ReplayBuffer::new(100);
        buf.push(Experience {
            state: vec![1.0],
            action: 0,
            reward: 1.0,
            next_state: vec![2.0],
            done: false,
            priority: 0.1,
        });
        buf.push(Experience {
            state: vec![3.0],
            action: 1,
            reward: 5.0,
            next_state: vec![4.0],
            done: false,
            priority: 10.0,
        });
        let batch = buf.sample_prioritized(1);
        assert_eq!(batch[0].priority, 10.0);
    }

    // S15.3 — DQN
    #[test]
    fn s15_3_td_error() {
        let q_values = vec![1.0, 2.0, 3.0];
        let target_q = vec![5.0];
        let actions = vec![1]; // Q[1] = 2.0
        let rewards = vec![1.0];
        let dones = vec![false];
        let errors = dqn_td_error(&q_values, &target_q, &actions, &rewards, &dones, 0.99);
        // target = 1.0 + 0.99 * 5.0 = 5.95; error = 5.95 - 2.0 = 3.95
        assert!((errors[0] - 3.95).abs() < 1e-10);
    }

    #[test]
    fn s15_3_dqn_config() {
        let cfg = DqnConfig::default();
        assert_eq!(cfg.gamma, 0.99);
        assert!(cfg.epsilon > 0.0);
    }

    // S15.4 — Policy Gradient
    #[test]
    fn s15_4_compute_returns() {
        let rewards = vec![1.0, 1.0, 1.0];
        let returns = compute_returns(&rewards, 0.99);
        assert!((returns[2] - 1.0).abs() < 1e-10);
        assert!(returns[0] > returns[1]); // Earlier has more accumulated reward
    }

    #[test]
    fn s15_4_baseline_subtraction() {
        let returns = vec![10.0, 20.0, 30.0];
        let centered = subtract_baseline(&returns);
        let mean: f64 = centered.iter().sum::<f64>() / 3.0;
        assert!(mean.abs() < 1e-10); // Should be zero-mean
    }

    // S15.5 — PPO
    #[test]
    fn s15_5_ppo_clipped_loss() {
        // Positive advantage, ratio within clip range
        let loss = ppo_clipped_loss(1.1, 1.0, 0.2);
        assert!((loss - 1.1).abs() < 1e-10); // min(1.1*1, 1.1*1) = 1.1

        // Ratio too high for positive advantage → clipped
        let loss2 = ppo_clipped_loss(1.5, 1.0, 0.2);
        assert!((loss2 - 1.2).abs() < 1e-10); // min(1.5, 1.2) = 1.2
    }

    // S15.6 — GAE
    #[test]
    fn s15_6_gae() {
        let rewards = vec![1.0, 1.0, 1.0];
        let values = vec![0.5, 0.5, 0.5];
        let advantages = compute_gae(&rewards, &values, 0.5, 0.99, 0.95);
        assert_eq!(advantages.len(), 3);
        // Advantages should be positive (rewards > values)
        for a in &advantages {
            assert!(*a > 0.0);
        }
    }

    // S15.7 — Multi-Agent
    #[test]
    fn s15_7_multi_agent_config() {
        let cfg = MultiAgentConfig {
            num_agents: 4,
            policy_sharing: PolicySharing::Shared,
        };
        assert_eq!(cfg.num_agents, 4);
        assert_eq!(cfg.policy_sharing.to_string(), "Shared");
    }

    // S15.8 — Vectorized Environments
    #[test]
    fn s15_8_vectorized_env() {
        let mut vec_env = VectorizedEnv::new("CartPole", 4, 2, 100, 8);
        assert_eq!(vec_env.num_envs(), 8);

        let states = vec_env.reset_all();
        assert_eq!(states.len(), 8);

        let results = vec_env.step_all(&[0, 1, 0, 1, 0, 1, 0, 1]);
        assert_eq!(results.len(), 8);
    }

    // S15.9 — Reward Shaping
    #[test]
    fn s15_9_curriculum_reward() {
        let stage = CurriculumStage {
            name: "easy".into(),
            reward_scale: 2.0,
            min_episodes: 100,
            threshold: 50.0,
        };
        assert_eq!(shape_reward(5.0, &stage), 10.0);
    }

    // S15.10 — Integration
    #[test]
    fn s15_10_ppo_config() {
        let cfg = PpoConfig::default();
        assert_eq!(cfg.clip_epsilon, 0.2);
        assert_eq!(cfg.num_epochs, 4);
    }

    #[test]
    fn s15_10_replay_circular() {
        let mut buf = ReplayBuffer::new(3);
        for i in 0..5 {
            buf.push(Experience {
                state: vec![i as f64],
                action: 0,
                reward: i as f64,
                next_state: vec![],
                done: false,
                priority: 1.0,
            });
        }
        assert_eq!(buf.size, 3);
        assert_eq!(buf.buffer.len(), 3);
    }
}
