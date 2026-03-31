//! Sprint W10: Full-Stack Web App — HTTP backend, static files, template engine,
//! database layer, auth/JWT, API router, request validator, integration tests.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W10.1: HttpBackend — WASI HTTP Server with REST API
// ═══════════════════════════════════════════════════════════════════════

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    /// GET request.
    Get,
    /// POST request.
    Post,
    /// PUT request.
    Put,
    /// DELETE request.
    Delete,
    /// PATCH request.
    Patch,
    /// OPTIONS request.
    Options,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Options => write!(f, "OPTIONS"),
        }
    }
}

impl HttpMethod {
    /// Parse from string.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "PATCH" => Ok(HttpMethod::Patch),
            "OPTIONS" => Ok(HttpMethod::Options),
            other => Err(format!("unknown HTTP method: {other}")),
        }
    }
}

/// HTTP request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Request method.
    pub method: HttpMethod,
    /// Request path.
    pub path: String,
    /// Headers.
    pub headers: HashMap<String, String>,
    /// Request body.
    pub body: String,
    /// Path parameters (from route matching).
    pub params: HashMap<String, String>,
    /// Query parameters.
    pub query: HashMap<String, String>,
}

impl HttpRequest {
    /// Create a GET request.
    pub fn get(path: &str) -> Self {
        Self {
            method: HttpMethod::Get,
            path: path.into(),
            headers: HashMap::new(),
            body: String::new(),
            params: HashMap::new(),
            query: HashMap::new(),
        }
    }

    /// Create a POST request with body.
    pub fn post(path: &str, body: &str) -> Self {
        Self {
            method: HttpMethod::Post,
            path: path.into(),
            headers: HashMap::new(),
            body: body.into(),
            params: HashMap::new(),
            query: HashMap::new(),
        }
    }

    /// Set a header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_lowercase(), value.into());
        self
    }

    /// Set a query parameter.
    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query.insert(key.into(), value.into());
        self
    }

    /// Get the Content-Type header.
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(|s| s.as_str())
    }

    /// Get the Authorization header.
    pub fn authorization(&self) -> Option<&str> {
        self.headers.get("authorization").map(|s| s.as_str())
    }
}

/// HTTP status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// 200 OK.
    pub const OK: Self = Self(200);
    /// 201 Created.
    pub const CREATED: Self = Self(201);
    /// 204 No Content.
    pub const NO_CONTENT: Self = Self(204);
    /// 400 Bad Request.
    pub const BAD_REQUEST: Self = Self(400);
    /// 401 Unauthorized.
    pub const UNAUTHORIZED: Self = Self(401);
    /// 403 Forbidden.
    pub const FORBIDDEN: Self = Self(403);
    /// 404 Not Found.
    pub const NOT_FOUND: Self = Self(404);
    /// 500 Internal Server Error.
    pub const INTERNAL_ERROR: Self = Self(500);

    /// Get the reason phrase.
    pub fn reason(&self) -> &'static str {
        match self.0 {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown",
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.0, self.reason())
    }
}

/// HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code.
    pub status: StatusCode,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Response body.
    pub body: String,
}

impl HttpResponse {
    /// Create a response with status and body.
    pub fn new(status: StatusCode, body: &str) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: body.into(),
        }
    }

    /// Create a 200 OK response.
    pub fn ok(body: &str) -> Self {
        Self::new(StatusCode::OK, body)
    }

    /// Create a 201 Created response.
    pub fn created(body: &str) -> Self {
        Self::new(StatusCode::CREATED, body)
    }

    /// Create a 404 Not Found response.
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "Not Found")
    }

    /// Create a 400 Bad Request response.
    pub fn bad_request(msg: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }

    /// Create a 401 Unauthorized response.
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized")
    }

    /// Create a JSON response.
    pub fn json(status: StatusCode, json_body: &str) -> Self {
        let mut resp = Self::new(status, json_body);
        resp.headers
            .insert("content-type".into(), "application/json".into());
        resp
    }

    /// Set a response header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_lowercase(), value.into());
        self
    }

    /// Is this a success response (2xx)?
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.2: StaticFileServer — Serve Files from Directory
// ═══════════════════════════════════════════════════════════════════════

/// MIME type mapping.
pub struct MimeType;

impl MimeType {
    /// Get MIME type from file extension.
    pub fn from_extension(ext: &str) -> &'static str {
        match ext.to_lowercase().as_str() {
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "txt" => "text/plain",
            "wasm" => "application/wasm",
            _ => "application/octet-stream",
        }
    }

    /// Extract extension from a path.
    pub fn extension(path: &str) -> &str {
        path.rsplit('.').next().unwrap_or("")
    }
}

/// Simulated static file server (in-memory filesystem).
#[derive(Debug, Default)]
pub struct StaticFileServer {
    /// Root directory path.
    pub root: String,
    /// In-memory files: path -> content.
    files: HashMap<String, Vec<u8>>,
}

impl StaticFileServer {
    /// Create a new static file server.
    pub fn new(root: &str) -> Self {
        Self {
            root: root.into(),
            files: HashMap::new(),
        }
    }

    /// Add a file to the in-memory filesystem.
    pub fn add_file(&mut self, path: &str, content: &[u8]) {
        self.files.insert(path.into(), content.to_vec());
    }

    /// Add a text file.
    pub fn add_text_file(&mut self, path: &str, content: &str) {
        self.files.insert(path.into(), content.as_bytes().to_vec());
    }

    /// Serve a request for a static file.
    pub fn serve(&self, request_path: &str) -> HttpResponse {
        // Normalize path
        let normalized = if request_path == "/" {
            "/index.html"
        } else {
            request_path
        };

        let full_path = format!("{}{normalized}", self.root);

        match self.files.get(&full_path) {
            Some(content) => {
                let ext = MimeType::extension(normalized);
                let mime = MimeType::from_extension(ext);
                let body = String::from_utf8_lossy(content).into_owned();
                HttpResponse::ok(&body).with_header("content-type", mime)
            }
            None => HttpResponse::not_found(),
        }
    }

    /// Number of files served.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.3: TemplateEngine — HTML Template Rendering
// ═══════════════════════════════════════════════════════════════════════

/// Template context (variable name -> value).
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    /// Variables.
    vars: HashMap<String, String>,
    /// List variables (for loops).
    lists: HashMap<String, Vec<HashMap<String, String>>>,
}

impl TemplateContext {
    /// Create a new context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable.
    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    /// Set a list variable for iteration.
    pub fn set_list(mut self, key: &str, items: Vec<HashMap<String, String>>) -> Self {
        self.lists.insert(key.into(), items);
        self
    }
}

/// Simple HTML template engine.
///
/// Supports:
/// - `{{ variable }}` — variable substitution
/// - `{% for item in list %}...{% endfor %}` — loops
/// - `{% if condition %}...{% endif %}` — conditionals (variable is truthy)
pub struct TemplateEngine;

impl TemplateEngine {
    /// Render a template with the given context.
    pub fn render(template: &str, ctx: &TemplateContext) -> Result<String, String> {
        let mut output = String::new();
        let mut remaining = template;

        while !remaining.is_empty() {
            // Check for template tags
            if let Some(pos) = remaining.find("{%") {
                // Output text before the tag
                output.push_str(&remaining[..pos]);
                remaining = &remaining[pos..];

                if let Some(end) = remaining.find("%}") {
                    let tag_content = remaining[2..end].trim();
                    remaining = &remaining[end + 2..];

                    if let Some(for_content) = tag_content.strip_prefix("for ") {
                        // Parse: "item in list"
                        let parts: Vec<&str> = for_content.split(" in ").collect();
                        if parts.len() != 2 {
                            return Err(format!("invalid for syntax: {for_content}"));
                        }
                        let item_name = parts[0].trim();
                        let list_name = parts[1].trim();

                        // Find endfor
                        let endfor_tag = "{% endfor %}";
                        let body_end = remaining
                            .find(endfor_tag)
                            .ok_or_else(|| "missing {% endfor %}".to_string())?;
                        let body = &remaining[..body_end];
                        remaining = &remaining[body_end + endfor_tag.len()..];

                        // Iterate
                        if let Some(items) = ctx.lists.get(list_name) {
                            for item in items {
                                let mut item_ctx = ctx.clone();
                                for (k, v) in item {
                                    item_ctx
                                        .vars
                                        .insert(format!("{item_name}.{k}"), v.clone());
                                }
                                let rendered = Self::render(body, &item_ctx)?;
                                output.push_str(&rendered);
                            }
                        }
                    } else if let Some(cond) = tag_content.strip_prefix("if ") {
                        let var_name = cond.trim();

                        // Find endif
                        let endif_tag = "{% endif %}";
                        let body_end = remaining
                            .find(endif_tag)
                            .ok_or_else(|| "missing {% endif %}".to_string())?;
                        let body = &remaining[..body_end];
                        remaining = &remaining[body_end + endif_tag.len()..];

                        // Check condition (truthy if variable exists and is not empty/"false"/"0")
                        let is_truthy = ctx
                            .vars
                            .get(var_name)
                            .map(|v| !v.is_empty() && v != "false" && v != "0")
                            .unwrap_or(false);

                        if is_truthy {
                            let rendered = Self::render(body, ctx)?;
                            output.push_str(&rendered);
                        }
                    }
                } else {
                    return Err("unclosed template tag {%".into());
                }
            } else if let Some(pos) = remaining.find("{{") {
                output.push_str(&remaining[..pos]);
                remaining = &remaining[pos..];

                if let Some(end) = remaining.find("}}") {
                    let var_name = remaining[2..end].trim();
                    let value = ctx.vars.get(var_name).cloned().unwrap_or_default();
                    output.push_str(&value);
                    remaining = &remaining[end + 2..];
                } else {
                    return Err("unclosed template variable {{".into());
                }
            } else {
                output.push_str(remaining);
                break;
            }
        }

        Ok(output)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.4: DatabaseLayer — SQLite Integration for Persistence
// ═══════════════════════════════════════════════════════════════════════

/// A simple in-memory database layer simulating SQLite.
#[derive(Debug, Default)]
pub struct DatabaseLayer {
    /// Tables: name -> rows (each row is a map of column -> value).
    tables: HashMap<String, Vec<HashMap<String, String>>>,
    /// Auto-increment counters per table.
    auto_ids: HashMap<String, u64>,
}

impl DatabaseLayer {
    /// Create a new database layer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a table.
    pub fn create_table(&mut self, name: &str) {
        self.tables.entry(name.into()).or_default();
        self.auto_ids.entry(name.into()).or_insert(0);
    }

    /// Insert a row, returning the auto-generated ID.
    pub fn insert(&mut self, table: &str, mut row: HashMap<String, String>) -> Result<u64, String> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| format!("table '{table}' not found"))?;
        let counter = self.auto_ids.get_mut(table).ok_or("no counter")?;
        *counter += 1;
        let id = *counter;
        row.insert("id".into(), id.to_string());
        rows.push(row);
        Ok(id)
    }

    /// Find all rows in a table.
    pub fn find_all(&self, table: &str) -> Result<&[HashMap<String, String>], String> {
        self.tables
            .get(table)
            .map(|v| v.as_slice())
            .ok_or_else(|| format!("table '{table}' not found"))
    }

    /// Find a row by ID.
    pub fn find_by_id(&self, table: &str, id: &str) -> Result<Option<&HashMap<String, String>>, String> {
        let rows = self
            .tables
            .get(table)
            .ok_or_else(|| format!("table '{table}' not found"))?;
        Ok(rows.iter().find(|r| r.get("id").map(|v| v.as_str()) == Some(id)))
    }

    /// Update a row by ID.
    pub fn update(
        &mut self,
        table: &str,
        id: &str,
        updates: HashMap<String, String>,
    ) -> Result<bool, String> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| format!("table '{table}' not found"))?;
        if let Some(row) = rows.iter_mut().find(|r| r.get("id").map(|v| v.as_str()) == Some(id)) {
            for (k, v) in updates {
                row.insert(k, v);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete a row by ID.
    pub fn delete(&mut self, table: &str, id: &str) -> Result<bool, String> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| format!("table '{table}' not found"))?;
        let before = rows.len();
        rows.retain(|r| r.get("id").map(|v| v.as_str()) != Some(id));
        Ok(rows.len() < before)
    }

    /// Count rows in a table.
    pub fn count(&self, table: &str) -> Result<usize, String> {
        self.tables
            .get(table)
            .map(|v| v.len())
            .ok_or_else(|| format!("table '{table}' not found"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.5: AuthLayer — User Registration, Login, JWT Sessions
// ═══════════════════════════════════════════════════════════════════════

/// User account.
#[derive(Debug, Clone)]
pub struct User {
    /// User ID.
    pub id: u64,
    /// Username.
    pub username: String,
    /// Password hash (simplified — in production, use bcrypt/argon2).
    pub password_hash: String,
    /// User role.
    pub role: String,
}

/// JWT-like session token (simplified).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionToken {
    /// User ID.
    pub user_id: u64,
    /// Username.
    pub username: String,
    /// Token string.
    pub token: String,
    /// Expiry timestamp (unix seconds).
    pub expires_at: u64,
}

impl SessionToken {
    /// Check if the token is expired.
    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }
}

/// Authentication layer.
#[derive(Debug, Default)]
pub struct AuthLayer {
    /// Registered users.
    users: Vec<User>,
    /// Active sessions: token string -> session.
    sessions: HashMap<String, SessionToken>,
    /// Next user ID.
    next_id: u64,
    /// Token validity duration in seconds.
    pub token_ttl: u64,
}

impl AuthLayer {
    /// Create a new auth layer.
    pub fn new() -> Self {
        Self {
            users: Vec::new(),
            sessions: HashMap::new(),
            next_id: 1,
            token_ttl: 3600, // 1 hour default
        }
    }

    /// Simple hash function (for demo; NOT cryptographically secure).
    fn hash_password(password: &str) -> String {
        let hash: u64 = password
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(37).wrapping_add(b as u64));
        format!("hash-{hash:016x}")
    }

    /// Register a new user.
    pub fn register(&mut self, username: &str, password: &str, role: &str) -> Result<u64, String> {
        if self.users.iter().any(|u| u.username == username) {
            return Err(format!("username '{username}' already taken"));
        }
        let id = self.next_id;
        self.next_id += 1;
        self.users.push(User {
            id,
            username: username.into(),
            password_hash: Self::hash_password(password),
            role: role.into(),
        });
        Ok(id)
    }

    /// Login and create a session token.
    pub fn login(&mut self, username: &str, password: &str, now: u64) -> Result<SessionToken, String> {
        let user = self
            .users
            .iter()
            .find(|u| u.username == username)
            .ok_or_else(|| "invalid credentials".to_string())?;

        if user.password_hash != Self::hash_password(password) {
            return Err("invalid credentials".into());
        }

        let token_str = format!(
            "jwt-{}-{}-{:016x}",
            user.id,
            now,
            (user.id as u64).wrapping_mul(now).wrapping_add(12345)
        );

        let token = SessionToken {
            user_id: user.id,
            username: user.username.clone(),
            token: token_str.clone(),
            expires_at: now + self.token_ttl,
        };

        self.sessions.insert(token_str, token.clone());
        Ok(token)
    }

    /// Validate a token, returning the session if valid.
    pub fn validate_token(&self, token: &str, now: u64) -> Result<&SessionToken, String> {
        let session = self
            .sessions
            .get(token)
            .ok_or_else(|| "invalid token".to_string())?;
        if session.is_expired(now) {
            return Err("token expired".into());
        }
        Ok(session)
    }

    /// Logout (invalidate token).
    pub fn logout(&mut self, token: &str) -> bool {
        self.sessions.remove(token).is_some()
    }

    /// Get user by ID.
    pub fn get_user(&self, id: u64) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }

    /// Number of registered users.
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    /// Number of active sessions.
    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.6: ApiRouter — RESTful Resource Routing
// ═══════════════════════════════════════════════════════════════════════

/// A route definition.
#[derive(Debug, Clone)]
pub struct Route {
    /// HTTP method.
    pub method: HttpMethod,
    /// Path pattern (e.g., "/api/users/:id").
    pub pattern: String,
    /// Handler name (used for dispatch).
    pub handler_name: String,
}

/// Route match result.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Matched handler name.
    pub handler_name: String,
    /// Path parameters extracted from the pattern.
    pub params: HashMap<String, String>,
}

/// RESTful API router with pattern matching.
#[derive(Debug, Default)]
pub struct ApiRouter {
    /// Registered routes.
    routes: Vec<Route>,
}

impl ApiRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a route.
    pub fn route(mut self, method: HttpMethod, pattern: &str, handler_name: &str) -> Self {
        self.routes.push(Route {
            method,
            pattern: pattern.into(),
            handler_name: handler_name.into(),
        });
        self
    }

    /// Convenience: register GET route.
    pub fn get(self, pattern: &str, handler: &str) -> Self {
        self.route(HttpMethod::Get, pattern, handler)
    }

    /// Convenience: register POST route.
    pub fn post(self, pattern: &str, handler: &str) -> Self {
        self.route(HttpMethod::Post, pattern, handler)
    }

    /// Convenience: register PUT route.
    pub fn put(self, pattern: &str, handler: &str) -> Self {
        self.route(HttpMethod::Put, pattern, handler)
    }

    /// Convenience: register DELETE route.
    pub fn delete(self, pattern: &str, handler: &str) -> Self {
        self.route(HttpMethod::Delete, pattern, handler)
    }

    /// Match a request to a route.
    pub fn match_route(&self, method: HttpMethod, path: &str) -> Option<RouteMatch> {
        for route in &self.routes {
            if route.method != method {
                continue;
            }
            if let Some(params) = Self::match_pattern(&route.pattern, path) {
                return Some(RouteMatch {
                    handler_name: route.handler_name.clone(),
                    params,
                });
            }
        }
        None
    }

    /// Match a path against a pattern, extracting parameters.
    fn match_pattern(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return None;
        }

        let mut params = HashMap::new();
        for (pat, actual) in pattern_parts.iter().zip(path_parts.iter()) {
            if let Some(param_name) = pat.strip_prefix(':') {
                params.insert(param_name.into(), (*actual).into());
            } else if pat != actual {
                return None;
            }
        }
        Some(params)
    }

    /// Number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// List all routes as (method, pattern) pairs.
    pub fn list_routes(&self) -> Vec<(HttpMethod, &str)> {
        self.routes
            .iter()
            .map(|r| (r.method, r.pattern.as_str()))
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.7: RequestValidator — Validate Request Body Schema
// ═══════════════════════════════════════════════════════════════════════

/// Validation rule for a field.
#[derive(Debug, Clone)]
pub enum ValidationRule {
    /// Field is required.
    Required,
    /// Minimum string length.
    MinLength(usize),
    /// Maximum string length.
    MaxLength(usize),
    /// Value must match a pattern (simplified — exact match for now).
    Pattern(String),
    /// Value must be one of the given options.
    OneOf(Vec<String>),
}

/// A field validation definition.
#[derive(Debug, Clone)]
pub struct FieldValidation {
    /// Field name.
    pub field: String,
    /// Rules to apply.
    pub rules: Vec<ValidationRule>,
}

/// Request body validation error.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Error message.
    pub message: String,
}

impl fmt::Display for RequestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Validates request body fields.
pub struct RequestValidator {
    /// Field validations.
    validations: Vec<FieldValidation>,
}

impl RequestValidator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self {
            validations: Vec::new(),
        }
    }

    /// Add a field validation.
    pub fn field(mut self, name: &str, rules: Vec<ValidationRule>) -> Self {
        self.validations.push(FieldValidation {
            field: name.into(),
            rules,
        });
        self
    }

    /// Validate a set of fields (key-value pairs).
    pub fn validate(
        &self,
        fields: &HashMap<String, String>,
    ) -> Result<(), Vec<RequestValidationError>> {
        let mut errors = Vec::new();

        for validation in &self.validations {
            let value = fields.get(&validation.field);

            for rule in &validation.rules {
                match rule {
                    ValidationRule::Required => {
                        if value.map(|v| v.is_empty()).unwrap_or(true) {
                            errors.push(RequestValidationError {
                                field: validation.field.clone(),
                                message: "is required".into(),
                            });
                        }
                    }
                    ValidationRule::MinLength(min) => {
                        if let Some(v) = value {
                            if v.len() < *min {
                                errors.push(RequestValidationError {
                                    field: validation.field.clone(),
                                    message: format!("must be at least {min} characters"),
                                });
                            }
                        }
                    }
                    ValidationRule::MaxLength(max) => {
                        if let Some(v) = value {
                            if v.len() > *max {
                                errors.push(RequestValidationError {
                                    field: validation.field.clone(),
                                    message: format!("must be at most {max} characters"),
                                });
                            }
                        }
                    }
                    ValidationRule::Pattern(pat) => {
                        if let Some(v) = value {
                            if !v.contains(pat.as_str()) {
                                errors.push(RequestValidationError {
                                    field: validation.field.clone(),
                                    message: format!("must match pattern '{pat}'"),
                                });
                            }
                        }
                    }
                    ValidationRule::OneOf(options) => {
                        if let Some(v) = value {
                            if !options.iter().any(|o| o == v) {
                                errors.push(RequestValidationError {
                                    field: validation.field.clone(),
                                    message: format!(
                                        "must be one of: {}",
                                        options.join(", ")
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for RequestValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W10.8: FullStackTestSuite — Integration Test for Full Request Cycle
// ═══════════════════════════════════════════════════════════════════════

/// A complete full-stack application wiring all components together.
#[derive(Debug)]
pub struct FullStackApp {
    /// API router.
    pub router: ApiRouter,
    /// Database layer.
    pub db: DatabaseLayer,
    /// Auth layer.
    pub auth: AuthLayer,
    /// Template engine context defaults.
    pub default_context: TemplateContext,
}

impl FullStackApp {
    /// Create a new full-stack app with standard configuration.
    pub fn new() -> Self {
        let router = ApiRouter::new()
            .get("/api/users", "list_users")
            .post("/api/users", "create_user")
            .get("/api/users/:id", "get_user")
            .put("/api/users/:id", "update_user")
            .delete("/api/users/:id", "delete_user")
            .post("/api/auth/login", "login")
            .post("/api/auth/register", "register");

        let mut db = DatabaseLayer::new();
        db.create_table("users");

        let auth = AuthLayer::new();

        Self {
            router,
            db,
            auth,
            default_context: TemplateContext::new().set("app_name", "Fajar App"),
        }
    }

    /// Handle a request through the full stack.
    pub fn handle(&mut self, req: &HttpRequest) -> HttpResponse {
        // Route matching
        let route = match self.router.match_route(req.method, &req.path) {
            Some(r) => r,
            None => return HttpResponse::not_found(),
        };

        match route.handler_name.as_str() {
            "list_users" => {
                let rows = self.db.find_all("users").unwrap_or(&[]);
                let body = format!("{{\"count\":{}}}", rows.len());
                HttpResponse::json(StatusCode::OK, &body)
            }
            "create_user" => {
                let mut row = HashMap::new();
                row.insert("data".into(), req.body.clone());
                match self.db.insert("users", row) {
                    Ok(id) => {
                        HttpResponse::json(StatusCode::CREATED, &format!("{{\"id\":{id}}}"))
                    }
                    Err(e) => HttpResponse::bad_request(&e),
                }
            }
            "get_user" => {
                let id = route.params.get("id").map(|s| s.as_str()).unwrap_or("0");
                match self.db.find_by_id("users", id) {
                    Ok(Some(_row)) => {
                        HttpResponse::json(StatusCode::OK, &format!("{{\"id\":{id}}}"))
                    }
                    Ok(None) => HttpResponse::not_found(),
                    Err(e) => HttpResponse::bad_request(&e),
                }
            }
            "delete_user" => {
                let id = route.params.get("id").map(|s| s.as_str()).unwrap_or("0");
                match self.db.delete("users", id) {
                    Ok(true) => HttpResponse::new(StatusCode::NO_CONTENT, ""),
                    Ok(false) => HttpResponse::not_found(),
                    Err(e) => HttpResponse::bad_request(&e),
                }
            }
            "register" => {
                // Simplified: use body as "username:password"
                let parts: Vec<&str> = req.body.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return HttpResponse::bad_request("body must be username:password");
                }
                match self.auth.register(parts[0], parts[1], "user") {
                    Ok(id) => HttpResponse::json(
                        StatusCode::CREATED,
                        &format!("{{\"user_id\":{id}}}"),
                    ),
                    Err(e) => HttpResponse::bad_request(&e),
                }
            }
            "login" => {
                let parts: Vec<&str> = req.body.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return HttpResponse::bad_request("body must be username:password");
                }
                match self.auth.login(parts[0], parts[1], 1000) {
                    Ok(token) => HttpResponse::json(
                        StatusCode::OK,
                        &format!("{{\"token\":\"{}\"}}", token.token),
                    ),
                    Err(e) => HttpResponse::unauthorized().with_header("x-error", &e),
                }
            }
            _ => HttpResponse::not_found(),
        }
    }
}

impl Default for FullStackApp {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W10.1: HttpBackend
    #[test]
    fn w10_1_http_request_get() {
        let req = HttpRequest::get("/api/users").with_header("Accept", "application/json");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.path, "/api/users");
    }

    #[test]
    fn w10_1_http_response_json() {
        let resp = HttpResponse::json(StatusCode::OK, r#"{"ok":true}"#);
        assert!(resp.is_success());
        assert_eq!(
            resp.headers.get("content-type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn w10_1_status_codes() {
        assert_eq!(StatusCode::OK.reason(), "OK");
        assert_eq!(StatusCode::NOT_FOUND.reason(), "Not Found");
        assert_eq!(format!("{}", StatusCode::CREATED), "201 Created");
    }

    #[test]
    fn w10_1_http_method_parse() {
        assert_eq!(HttpMethod::from_str("GET").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str("post").unwrap(), HttpMethod::Post);
        assert!(HttpMethod::from_str("INVALID").is_err());
    }

    // W10.2: StaticFileServer
    #[test]
    fn w10_2_static_serve() {
        let mut server = StaticFileServer::new("/static");
        server.add_text_file("/static/index.html", "<h1>Hello</h1>");
        server.add_text_file("/static/style.css", "body { color: red }");

        let resp = server.serve("/index.html");
        assert_eq!(resp.status, StatusCode::OK);
        assert!(resp.body.contains("<h1>Hello</h1>"));
        assert_eq!(
            resp.headers.get("content-type"),
            Some(&"text/html".to_string())
        );
    }

    #[test]
    fn w10_2_static_not_found() {
        let server = StaticFileServer::new("/static");
        let resp = server.serve("/missing.html");
        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn w10_2_mime_types() {
        assert_eq!(MimeType::from_extension("html"), "text/html");
        assert_eq!(MimeType::from_extension("js"), "application/javascript");
        assert_eq!(MimeType::from_extension("png"), "image/png");
        assert_eq!(MimeType::from_extension("wasm"), "application/wasm");
    }

    // W10.3: TemplateEngine
    #[test]
    fn w10_3_template_variable() {
        let ctx = TemplateContext::new().set("name", "Fajar");
        let result = TemplateEngine::render("Hello {{ name }}!", &ctx).unwrap();
        assert_eq!(result, "Hello Fajar!");
    }

    #[test]
    fn w10_3_template_for_loop() {
        let items = vec![
            {
                let mut m = HashMap::new();
                m.insert("name".into(), "Alice".into());
                m
            },
            {
                let mut m = HashMap::new();
                m.insert("name".into(), "Bob".into());
                m
            },
        ];
        let ctx = TemplateContext::new().set_list("users", items);
        let tmpl = "{% for u in users %}{{ u.name }} {% endfor %}";
        let result = TemplateEngine::render(tmpl, &ctx).unwrap();
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
    }

    #[test]
    fn w10_3_template_if_truthy() {
        let ctx = TemplateContext::new().set("admin", "true");
        let result =
            TemplateEngine::render("{% if admin %}Admin{% endif %}", &ctx).unwrap();
        assert_eq!(result, "Admin");
    }

    #[test]
    fn w10_3_template_if_falsy() {
        let ctx = TemplateContext::new().set("admin", "false");
        let result =
            TemplateEngine::render("{% if admin %}Admin{% endif %}", &ctx).unwrap();
        assert_eq!(result, "");
    }

    // W10.4: DatabaseLayer
    #[test]
    fn w10_4_db_crud() {
        let mut db = DatabaseLayer::new();
        db.create_table("items");

        let mut row = HashMap::new();
        row.insert("name".into(), "widget".into());
        let id = db.insert("items", row).unwrap();
        assert_eq!(id, 1);

        assert_eq!(db.count("items").unwrap(), 1);

        let found = db.find_by_id("items", "1").unwrap().unwrap();
        assert_eq!(found.get("name"), Some(&"widget".to_string()));

        let mut updates = HashMap::new();
        updates.insert("name".into(), "gadget".into());
        assert!(db.update("items", "1", updates).unwrap());

        assert!(db.delete("items", "1").unwrap());
        assert_eq!(db.count("items").unwrap(), 0);
    }

    #[test]
    fn w10_4_db_table_not_found() {
        let db = DatabaseLayer::new();
        assert!(db.find_all("nonexistent").is_err());
    }

    // W10.5: AuthLayer
    #[test]
    fn w10_5_auth_register_login() {
        let mut auth = AuthLayer::new();
        let id = auth.register("fajar", "secret123", "admin").unwrap();
        assert_eq!(id, 1);

        let token = auth.login("fajar", "secret123", 1000).unwrap();
        assert_eq!(token.user_id, 1);
        assert!(!token.is_expired(1000));

        let session = auth.validate_token(&token.token, 1500).unwrap();
        assert_eq!(session.username, "fajar");
    }

    #[test]
    fn w10_5_auth_bad_password() {
        let mut auth = AuthLayer::new();
        auth.register("alice", "pass", "user").unwrap();
        assert!(auth.login("alice", "wrong", 1000).is_err());
    }

    #[test]
    fn w10_5_auth_expired_token() {
        let mut auth = AuthLayer::new();
        auth.token_ttl = 100;
        auth.register("bob", "pass", "user").unwrap();
        let token = auth.login("bob", "pass", 1000).unwrap();
        assert!(auth.validate_token(&token.token, 2000).is_err());
    }

    #[test]
    fn w10_5_auth_logout() {
        let mut auth = AuthLayer::new();
        auth.register("x", "y", "user").unwrap();
        let token = auth.login("x", "y", 0).unwrap();
        assert_eq!(auth.active_sessions(), 1);
        assert!(auth.logout(&token.token));
        assert_eq!(auth.active_sessions(), 0);
    }

    // W10.6: ApiRouter
    #[test]
    fn w10_6_router_exact_match() {
        let router = ApiRouter::new()
            .get("/api/users", "list_users")
            .post("/api/users", "create_user");
        let m = router.match_route(HttpMethod::Get, "/api/users").unwrap();
        assert_eq!(m.handler_name, "list_users");
        let m = router.match_route(HttpMethod::Post, "/api/users").unwrap();
        assert_eq!(m.handler_name, "create_user");
    }

    #[test]
    fn w10_6_router_param_extraction() {
        let router = ApiRouter::new().get("/api/users/:id", "get_user");
        let m = router
            .match_route(HttpMethod::Get, "/api/users/42")
            .unwrap();
        assert_eq!(m.handler_name, "get_user");
        assert_eq!(m.params.get("id"), Some(&"42".to_string()));
    }

    #[test]
    fn w10_6_router_no_match() {
        let router = ApiRouter::new().get("/api/users", "list");
        assert!(router.match_route(HttpMethod::Get, "/api/posts").is_none());
        assert!(router
            .match_route(HttpMethod::Post, "/api/users")
            .is_none());
    }

    // W10.7: RequestValidator
    #[test]
    fn w10_7_validation_pass() {
        let validator = RequestValidator::new()
            .field("name", vec![ValidationRule::Required, ValidationRule::MinLength(2)])
            .field("role", vec![
                ValidationRule::OneOf(vec!["admin".into(), "user".into()]),
            ]);
        let mut fields = HashMap::new();
        fields.insert("name".into(), "Fajar".into());
        fields.insert("role".into(), "admin".into());
        assert!(validator.validate(&fields).is_ok());
    }

    #[test]
    fn w10_7_validation_fail() {
        let validator = RequestValidator::new()
            .field("name", vec![ValidationRule::Required])
            .field("email", vec![ValidationRule::Pattern("@".into())]);
        let mut fields = HashMap::new();
        fields.insert("name".into(), "".into());
        fields.insert("email".into(), "invalid".into());
        let errors = validator.validate(&fields).unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn w10_7_validation_max_length() {
        let validator = RequestValidator::new()
            .field("code", vec![ValidationRule::MaxLength(5)]);
        let mut fields = HashMap::new();
        fields.insert("code".into(), "toolong".into());
        assert!(validator.validate(&fields).is_err());
    }

    // W10.8: FullStackApp integration
    #[test]
    fn w10_8_fullstack_create_and_list() {
        let mut app = FullStackApp::new();

        // Create a user
        let req = HttpRequest::post("/api/users", "Alice");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::CREATED);
        assert!(resp.body.contains("\"id\":1"));

        // List users
        let req = HttpRequest::get("/api/users");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::OK);
        assert!(resp.body.contains("\"count\":1"));
    }

    #[test]
    fn w10_8_fullstack_auth_flow() {
        let mut app = FullStackApp::new();

        // Register
        let req = HttpRequest::post("/api/auth/register", "alice:password123");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::CREATED);

        // Login
        let req = HttpRequest::post("/api/auth/login", "alice:password123");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::OK);
        assert!(resp.body.contains("token"));

        // Bad login
        let req = HttpRequest::post("/api/auth/login", "alice:wrong");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn w10_8_fullstack_not_found() {
        let mut app = FullStackApp::new();
        let req = HttpRequest::get("/api/nonexistent");
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn w10_8_fullstack_delete_user() {
        let mut app = FullStackApp::new();

        // Create
        let req = HttpRequest::post("/api/users", "Bob");
        app.handle(&req);

        // Delete
        let req = HttpRequest {
            method: HttpMethod::Delete,
            path: "/api/users/1".into(),
            headers: HashMap::new(),
            body: String::new(),
            params: HashMap::new(),
            query: HashMap::new(),
        };
        let resp = app.handle(&req);
        assert_eq!(resp.status, StatusCode::NO_CONTENT);

        // Verify deleted
        let req = HttpRequest::get("/api/users");
        let resp = app.handle(&req);
        assert!(resp.body.contains("\"count\":0"));
    }
}
