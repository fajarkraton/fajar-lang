# tcp-echo-server — async networking example

A line-oriented TCP echo server demonstrating Fajar Lang's:
- `async` / `.await` syntax
- Per-connection task spawning via `spawn()`
- `Result<T, E>` propagation with `?`
- Multi-file project layout

## Layout

```
tcp-echo-server/
├── fj.toml
├── README.md
└── src/
    ├── main.fj      # entry: bind, accept, spawn handler
    └── handler.fj   # pub async fn echo_loop
```

## Build & run

```bash
cd examples/tcp-echo-server
fj build
fj run
# in another shell:
nc localhost 8080
> hello
hello
> world
world
> ^C
```

The server prints `served N connections` when shut down (e.g., via
SIGINT — `Ctrl-C` then accept-loop's first `?` propagates the error).

## What it demonstrates

- **`async fn`** propagates effect rows through the type system. The
  compiler emits `EE006` if `echo_loop` is called from `@kernel`
  context.
- **`spawn()`** creates an independent task. The returned future is
  fire-and-forget here; for join-style coordination see
  `docs/TUTORIAL.md` Chapter 7.
- **`?` operator** turns the recv/send `Result` into early-return on
  network errors.

## Extending

- Bound the connection count: replace `loop { ... }` with `for _ in 0..max_conns`.
- Add per-connection deadline: wrap `recv_line()` in `with_timeout()`.
- Switch to UDP: `UdpSocket::bind` instead of `TcpListener::bind`.

## Related

- `examples/recipes/chat_server.fj` — multi-client chat with broadcast.
- `examples/recipes/rest_api.fj` — HTTP/JSON REST handler on top of TCP.
- `docs/TUTORIAL.md` Chapter 7 — async + effects.
