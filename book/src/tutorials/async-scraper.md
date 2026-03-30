# Async Web Scraper

Build a concurrent web scraper using Fajar Lang's async/await with real tokio I/O.

## What You'll Build

An async scraper that:
- Fetches multiple web pages concurrently with `async_http_get`
- Extracts data using `regex_find_all`
- Uses `async_spawn` + `async_join` for parallel execution

## Step 1: Basic Async HTTP Request

```fajar
fn main() {
    println("=== Async Web Scraper ===")

    // Single async request
    let body = async_http_get("http://example.com").await
    println(f"Fetched {len(body)} bytes")
```

`async_http_get` returns a `Future<str>`. When you `.await` it, the real HTTP request happens via tokio's TCP stack.

## Step 2: Extract Data with Regex

```fajar
    // Find all links in the HTML
    let links = regex_find_all("<a href=\"[^\"]+\"", body)
    println(f"Found {len(links)} links")

    // Find title
    let title = regex_find("<title>[^<]+</title>", body)
    if title != null {
        println(f"Title: {title}")
    }
```

## Step 3: Concurrent Fetching

```fajar
    // Define fetch functions for different URLs
    fn fetch_page1() -> str {
        let body = async_http_get("http://example.com").await
        f"page1: {len(body)} bytes"
    }

    fn fetch_page2() -> str {
        let body = async_http_get("http://example.com/about").await
        f"page2: {len(body)} bytes"
    }

    // Spawn concurrent tasks
    let f1 = async_spawn("fetch_page1")
    let f2 = async_spawn("fetch_page2")

    // Wait for all to complete
    let results = async_join(f1, f2)
    println(f"Completed {len(results)} pages")
```

## Step 4: Add Rate Limiting

```fajar
    // Sleep between requests to be polite
    let mut i = 0
    let urls = ["http://example.com", "http://example.com/page2"]
    while i < len(urls) {
        let body = async_http_get(urls[i]).await
        println(f"  [{i}] {len(body)} bytes")

        // Wait 500ms between requests
        async_sleep(500).await
        i = i + 1
    }

    println("Scraping complete")
}
```

## Key Concepts

| Builtin | Purpose |
|---------|---------|
| `async_http_get(url)` | Fetch URL via real TCP (returns Future) |
| `async_http_post(url, body)` | POST request (returns Future) |
| `async_sleep(ms)` | Real sleep via tokio (returns Future) |
| `async_spawn(fn_name)` | Spawn function as concurrent task |
| `async_join(f1, f2, ...)` | Wait for all futures, return results |
| `async_select(f1, f2, ...)` | Return first completed result |
| `.await` | Resolve a Future to its value |
| `regex_find_all(pattern, text)` | Extract all matches |
| `regex_find(pattern, text)` | Extract first match |

## How Async Works in Fajar Lang

1. **`async_http_get(url)`** creates a `Future` and registers a real tokio async operation
2. **`.await`** checks if the Future is a real async op → executes via `tokio::runtime::Runtime::block_on()`
3. **User-defined `async fn`** uses cooperative evaluation (body executes on `.await`)
4. **`async_spawn`** creates a task that can be awaited later with `async_join`

The tokio runtime is lazily initialized on first async builtin call.

## Full Source

See [`examples/websocket_chat.fj`](https://github.com/fajarkraton/fajar-lang/blob/main/examples/websocket_chat.fj) for another async networking example.
