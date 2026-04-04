# Plan: Real RL DQN Agent — Sprint 4

> **Goal:** `rl_agent_step()` uses real neural network policy, not random walk
> **Module:** ml_advanced/reinforcement [sim] → [x]
> **Estimated LOC:** ~640
> **Risk:** MEDIUM

---

## Architecture

```
CartPole State [batch, 4]
        |
   Dense(4, 128)       weights: [4, 128], bias: [1, 128]
        |
     ReLU
        |
   Dense(128, 2)       weights: [128, 2], bias: [1, 2]
        |
   Q-values [batch, 2]   (one per action: left/right)
```

Total parameters: 898 (tiny, CPU-friendly)

---

## Critical Prerequisite

**`Dense::forward_tracked`** — Dense layer currently has NO tape integration.
Must add ~5 LOC composing `matmul_tracked + add_tracked`. This unblocks ALL gradient flow.

---

## New Structs

### CartPoleEnv (real physics, not random walk)
- State: `[x, x_dot, theta, theta_dot]` (4 dims)
- Actions: 0=left, 1=right (force ±10N)
- Physics: Euler integration, dt=0.02, gravity=9.8
- Terminates: |theta| > 12°, |x| > 2.4, or 200 steps
- Reward: +1.0 per step survived

### PolicyNetwork
- 2-layer Dense: `state_dim → 128 → action_dim`
- `forward()`, `forward_tracked()`, `parameters()`, `clone_from()`

### DqnAgent
- online_net + target_net (PolicyNetwork)
- replay_buffer (existing ReplayBuffer)
- optimizer (Adam, lr=1e-3)
- epsilon-greedy: ε=1.0 → 0.01 (decay 0.995/episode)
- target update every 10 episodes

---

## Training Loop

```
for episode in 0..300:
    state = env.reset()
    loop:
        action = agent.select_action(state)     // ε-greedy
        result = env.step(action)
        agent.store_experience(state, action, result)
        if replay.can_sample(32):
            agent.train_step(32)                 // MSE(Q_online, r + γ·max(Q_target))
        state = result.state
        if result.done: break
    agent.decay_epsilon()
    if episode % 10 == 0: agent.update_target()
```

**Success criteria:** mean(rewards[280..300]) > 150 (out of 200 max)

---

## The Gather Problem (DQN-specific)

DQN needs Q(s, a) for taken action. No tracked "gather" op exists.
**Solution:** One-hot action mask × Q-values, then matmul with ones vector to sum:
```
masked = mul_tracked(q_all, action_mask)     // zero out non-taken
q_taken = matmul_tracked(masked, ones)       // sum along action dim
```

---

## Files to Modify

| File | Changes | LOC |
|------|---------|-----|
| `src/ml_advanced/reinforcement.rs` | CartPoleEnv, PolicyNetwork, DqnAgent, train_dqn_cartpole | ~330 |
| `src/runtime/ml/layers.rs` | Dense::forward_tracked | ~5 |
| `src/interpreter/eval/builtins.rs` | Upgrade rl_agent_create/step, add rl_agent_train | ~140 |
| `src/interpreter/eval/mod.rs` | Remove from SIMULATED_BUILTINS, register names | ~5 |
| `tests/` | Unit + integration tests | ~150 |
| **Total** | | **~640** |

---

## Implementation Phases

**Phase 1 — Foundation:**
1. Dense::forward_tracked (5 LOC, unblocks everything)
2. ReplayBuffer::sample_random (true random sampling)
3. CartPoleEnv with real physics
4. Test: episodes terminate correctly

**Phase 2 — Network + Agent:**
5. PolicyNetwork struct + forward/forward_tracked
6. DqnAgent with select_action, train_step, update_target
7. Test: gradients flow through all 4 parameter tensors

**Phase 3 — Training:**
8. train_dqn_cartpole function
9. Test: mean(rewards[280..300]) > 150

**Phase 4 — Builtins:**
10. Wire rl_agent_create/step to real DqnAgent
11. Add rl_agent_train builtin
12. Remove from SIMULATED_BUILTINS

---

## Test Plan

| Test | Criteria |
|------|----------|
| CartPole physics | State transitions match known inputs |
| ε-greedy | ε=1.0 → random, ε=0.0 → greedy |
| Gradient flow | All 4 params get non-zero gradients after train_step |
| Target update | target_net weights = online_net weights after update |
| Loss decreases | 100 train_steps on fixed buffer → loss drops |
| **DQN learns** | **Seed=42, 300 episodes: mean(rewards[280..300]) > 150** |

---

*Ready for execution in 1 session*
