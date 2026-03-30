# Build a REST API in 5 Minutes

This tutorial shows how to build a complete CRUD REST API using Fajar Lang's HTTP server framework with SQLite persistence.

## What You'll Build

A user management API with:
- `GET /api/users` — list all users
- `GET /api/users/:id` — get user by ID
- `POST /api/users` — create user
- `DELETE /api/users/:id` — delete user
- Logging middleware on every request
- SQLite database for persistence

## Prerequisites

```bash
cargo build --release
```

## Step 1: Set Up the Database

```fajar
fn init_db() -> i64 {
    let db = db_open(":memory:")
    db_execute(db, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)")
    db_execute(db, "INSERT INTO users (name, email) VALUES ('Fajar', 'fajar@lang.dev')")
    println("[db] Initialized")
    db
}
```

The `db_open` builtin creates an in-memory SQLite database. For persistence, use a file path like `"app.db"`.

## Step 2: Define Route Handlers

Each handler receives `(method, path, body, params)` and returns a JSON string:

```fajar
fn handle_list(_m: str, _p: str, _b: str, _params: str) -> str {
    let rows = db_query(1, "SELECT id, name, email FROM users")
    let count = len(rows)
    f"{{\"users\": {count}, \"status\": \"ok\"}}"
}

fn handle_get(_m: str, path: str, _b: str, _params: str) -> str {
    let parts = path.split("/")
    let id = parts[len(parts) - 1]
    f"{{\"user_id\": \"{id}\", \"found\": true}}"
}

fn handle_create(_m: str, _p: str, body: str, _params: str) -> str {
    let data = request_json(body)
    f"{{\"created\": true}}"
}
```

## Step 3: Set Up the Server

```fajar
fn log_request(method: str, path: str, _body: str) {
    println(f"  [{method}] {path}")
}

fn main() {
    let db = init_db()
    let srv = http_server(8080)

    // Register middleware
    http_middleware(srv, "log_request")

    // Register routes
    http_route(srv, "GET", "/api/users", "handle_list")
    http_route(srv, "GET", "/api/users/:id", "handle_get")
    http_route(srv, "POST", "/api/users", "handle_create")

    println("[server] Starting on port 8080...")
    let served = http_start(srv, 10)
    println(f"Served {served} requests")
}
```

## Step 4: Test It

```bash
# Terminal 1: Start the server
fj run rest_api.fj

# Terminal 2: Send requests
curl http://127.0.0.1:8080/api/users
curl http://127.0.0.1:8080/api/users/1
curl -X POST -d '{"name":"Alice"}' http://127.0.0.1:8080/api/users
```

## Key Concepts

| Builtin | Purpose |
|---------|---------|
| `http_server(port)` | Create server on port |
| `http_route(srv, method, pattern, handler_fn)` | Register route with `:param` support |
| `http_middleware(srv, fn_name)` | Add middleware (runs before routing) |
| `http_start(srv, max)` | Start serving loop |
| `request_json(body)` | Parse JSON string to Map |
| `response_json(status, data)` | Format JSON response |
| `db_open(path)` | Open SQLite database |
| `db_execute(db, sql)` | Execute SQL statement |
| `db_query(db, sql)` | Query and return rows |

## Full Source

See [`examples/rest_api_crud.fj`](https://github.com/fajarkraton/fajar-lang/blob/main/examples/rest_api_crud.fj) for the complete working example.

## Next Steps

- Add HTTPS with `http_start_tls(srv, max, cert, key)` (`--features https`)
- Use `regex_match` for input validation
- Add authentication middleware
