# Advanced ML

Fajar Lang provides state-of-the-art ML architectures beyond basic neural networks.

## Transformer

```fajar
@device
fn transformer_block(input: Tensor, config: TransformerConfig) -> Tensor {
    // Multi-head self-attention
    let q = tensor_matmul(input, w_q)
    let k = tensor_matmul(input, w_k)
    let v = tensor_matmul(input, w_v)

    let attention = scaled_dot_product_attention(q, k, v)
    let attn_out = tensor_add(input, attention)  // Residual
    let normed = layer_norm(attn_out)

    // Feed-forward network
    let ff = tensor_relu(tensor_matmul(normed, w_ff1))
    let ff_out = tensor_matmul(ff, w_ff2)
    layer_norm(tensor_add(normed, ff_out))  // Residual
}
```

Features: multi-head attention, KV-cache for inference, causal masking, positional encoding.

## Diffusion Models

```fajar
@device
fn denoise(noisy: Tensor, timestep: i64, model: DiffusionModel) -> Tensor {
    // Predict noise at given timestep
    let predicted_noise = model.forward(noisy, timestep)

    // DDPM sampling step
    ddpm_step(noisy, predicted_noise, timestep, model.schedule)
}

fn generate_image(model: DiffusionModel) -> Tensor {
    let mut x = tensor_randn(1, 3, 64, 64)  // Start from noise

    let mut t = 1000
    while t > 0 {
        x = denoise(x, t, model)
        t = t - 1
    }
    x
}
```

Noise schedules: linear, cosine, sigmoid. Samplers: DDPM, DDIM.

## Reinforcement Learning

```fajar
@device
fn train_agent(env: Environment) {
    let agent = DqnAgent {
        replay_buffer_size: 10000,
        epsilon: 1.0,
        epsilon_decay: 0.995,
        gamma: 0.99,
    }

    let mut episode = 0
    while episode < 1000 {
        let mut state = env.reset()
        let mut done = false

        while !done {
            let action = agent.select_action(state)  // Epsilon-greedy
            let (next_state, reward, is_done) = env.step(action)
            agent.store_transition(state, action, reward, next_state)
            agent.learn()  // Sample from replay buffer
            state = next_state
            done = is_done
        }
        episode = episode + 1
    }
}
```

Algorithms: Policy Gradient (REINFORCE), DQN with replay buffer, epsilon-greedy exploration.

## Model Serving

```fajar
let server = ModelServer {
    model: load_model("model.onnx"),
    batch_size: 32,
    timeout_ms: 100,
}

server.start(8080)  // Batches requests for throughput
```

Features: request batching, health monitoring, latency tracking.
