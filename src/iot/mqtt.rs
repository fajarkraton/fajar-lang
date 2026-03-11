//! MQTT 3.1.1 client for Fajar Lang.
//!
//! Provides simulation stubs for an MQTT client supporting QoS 0/1,
//! topic subscriptions with wildcards, last will and testament, and
//! auto-reconnect with exponential backoff. Models the API without
//! requiring an actual MQTT broker or network connection.
//!
//! # API Groups
//!
//! - **Connection**: connect, disconnect, auto-reconnect
//! - **Publishing**: send messages with QoS 0 (at most once) or 1 (at least once)
//! - **Subscribing**: topic filters with `+` and `#` wildcards
//! - **Callbacks**: register message handler
//! - **Last Will**: configure message sent by broker on unclean disconnect
//!
//! # Usage
//!
//! ```rust
//! use fajar_lang::iot::mqtt::*;
//!
//! let config = MqttConfig::new("mqtt://localhost:1883", "fj-client-1");
//! let mut client = MqttClient::connect(config).unwrap();
//! client.subscribe("sensor/+/temperature", QoS::AtLeastOnce).unwrap();
//! client.publish("sensor/1/temperature", b"25.3", QoS::AtMostOnce).unwrap();
//! ```

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from MQTT operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MqttError {
    /// Client not connected to broker.
    #[error("MQTT not connected")]
    NotConnected,

    /// Client already connected.
    #[error("MQTT already connected to {broker_url}")]
    AlreadyConnected {
        /// Broker URL.
        broker_url: String,
    },

    /// Connection to broker failed.
    #[error("connection failed: {reason}")]
    ConnectionFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid broker URL.
    #[error("invalid broker URL: {reason}")]
    InvalidBrokerUrl {
        /// Reason the URL is invalid.
        reason: String,
    },

    /// Invalid client ID (empty or too long).
    #[error("invalid client ID: {reason}")]
    InvalidClientId {
        /// Reason the ID is invalid.
        reason: String,
    },

    /// Invalid topic name or filter.
    #[error("invalid topic: {reason}")]
    InvalidTopic {
        /// Reason the topic is invalid.
        reason: String,
    },

    /// Publish operation failed.
    #[error("publish failed: {reason}")]
    PublishFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Subscribe operation failed.
    #[error("subscribe failed: {reason}")]
    SubscribeFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Payload too large (max 256 MB per MQTT spec).
    #[error("payload too large: {size} bytes (max: {max})")]
    PayloadTooLarge {
        /// Actual size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum client ID length (per MQTT 3.1.1 spec).
pub const MAX_CLIENT_ID_LEN: usize = 23;

/// Maximum payload size in bytes (256 MB).
pub const MAX_PAYLOAD_SIZE: usize = 256 * 1024 * 1024;

/// Default keepalive interval in seconds.
pub const DEFAULT_KEEPALIVE_SECS: u16 = 60;

/// Default auto-reconnect initial delay in milliseconds.
pub const RECONNECT_INITIAL_MS: u64 = 1000;

/// Maximum auto-reconnect delay in milliseconds.
pub const RECONNECT_MAX_MS: u64 = 60_000;

// ═══════════════════════════════════════════════════════════════════════
// Data types
// ═══════════════════════════════════════════════════════════════════════

/// MQTT Quality of Service level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QoS {
    /// At most once (fire and forget).
    AtMostOnce,
    /// At least once (acknowledged delivery).
    AtLeastOnce,
}

impl QoS {
    /// Returns the numeric QoS level (0 or 1).
    pub fn level(self) -> u8 {
        match self {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce => 1,
        }
    }
}

impl std::fmt::Display for QoS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QoS::AtMostOnce => write!(f, "QoS0"),
            QoS::AtLeastOnce => write!(f, "QoS1"),
        }
    }
}

/// MQTT client configuration.
#[derive(Debug, Clone)]
pub struct MqttConfig {
    /// Broker URL (e.g., `mqtt://host:port` or `mqtts://host:port`).
    pub broker_url: String,
    /// Client identifier.
    pub client_id: String,
    /// Keepalive interval in seconds (0 = disabled).
    pub keepalive_secs: u16,
    /// Whether to start with a clean session.
    pub clean_session: bool,
    /// Last will and testament (optional).
    pub last_will: Option<LastWill>,
    /// Auto-reconnect initial delay in ms.
    pub reconnect_initial_ms: u64,
    /// Auto-reconnect maximum delay in ms.
    pub reconnect_max_ms: u64,
}

impl MqttConfig {
    /// Creates a new MQTT configuration with default settings.
    pub fn new(broker_url: &str, client_id: &str) -> Self {
        Self {
            broker_url: broker_url.to_string(),
            client_id: client_id.to_string(),
            keepalive_secs: DEFAULT_KEEPALIVE_SECS,
            clean_session: true,
            last_will: None,
            reconnect_initial_ms: RECONNECT_INITIAL_MS,
            reconnect_max_ms: RECONNECT_MAX_MS,
        }
    }

    /// Sets the keepalive interval.
    pub fn with_keepalive(mut self, secs: u16) -> Self {
        self.keepalive_secs = secs;
        self
    }

    /// Sets the clean session flag.
    pub fn with_clean_session(mut self, clean: bool) -> Self {
        self.clean_session = clean;
        self
    }

    /// Sets the last will and testament.
    pub fn with_last_will(mut self, will: LastWill) -> Self {
        self.last_will = Some(will);
        self
    }

    /// Sets auto-reconnect parameters.
    pub fn with_reconnect(mut self, initial_ms: u64, max_ms: u64) -> Self {
        self.reconnect_initial_ms = initial_ms;
        self.reconnect_max_ms = max_ms;
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), MqttError> {
        validate_broker_url(&self.broker_url)?;
        validate_client_id(&self.client_id)?;
        if let Some(ref will) = self.last_will {
            validate_topic(&will.topic)?;
        }
        Ok(())
    }
}

/// Last Will and Testament message.
///
/// Sent by the broker to subscribers if the client disconnects unexpectedly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    /// Topic to publish the will message on.
    pub topic: String,
    /// Will message payload.
    pub message: Vec<u8>,
    /// QoS level for the will message.
    pub qos: QoS,
    /// Whether the will message should be retained.
    pub retain: bool,
}

impl LastWill {
    /// Creates a new Last Will message.
    pub fn new(topic: &str, message: &[u8], qos: QoS, retain: bool) -> Self {
        Self {
            topic: topic.to_string(),
            message: message.to_vec(),
            qos,
            retain,
        }
    }
}

/// An MQTT message received from a subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttMessage {
    /// Topic the message was published on.
    pub topic: String,
    /// Message payload.
    pub payload: Vec<u8>,
    /// QoS level.
    pub qos: QoS,
    /// Whether the message is retained.
    pub retain: bool,
    /// Message identifier (non-zero for QoS 1).
    pub message_id: u16,
}

impl MqttMessage {
    /// Creates a new MQTT message.
    pub fn new(topic: &str, payload: &[u8], qos: QoS) -> Self {
        Self {
            topic: topic.to_string(),
            payload: payload.to_vec(),
            qos,
            retain: false,
            message_id: 0,
        }
    }

    /// Sets the retain flag.
    pub fn with_retain(mut self, retain: bool) -> Self {
        self.retain = retain;
        self
    }

    /// Returns the payload as a UTF-8 string, if valid.
    pub fn payload_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.payload).ok()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Validation helpers
// ═══════════════════════════════════════════════════════════════════════

/// Validates a broker URL.
fn validate_broker_url(url: &str) -> Result<(), MqttError> {
    if url.is_empty() {
        return Err(MqttError::InvalidBrokerUrl {
            reason: "broker URL cannot be empty".to_string(),
        });
    }
    if !url.starts_with("mqtt://") && !url.starts_with("mqtts://") {
        return Err(MqttError::InvalidBrokerUrl {
            reason: "broker URL must start with mqtt:// or mqtts://".to_string(),
        });
    }
    Ok(())
}

/// Validates an MQTT client ID.
fn validate_client_id(id: &str) -> Result<(), MqttError> {
    if id.is_empty() {
        return Err(MqttError::InvalidClientId {
            reason: "client ID cannot be empty".to_string(),
        });
    }
    if id.len() > MAX_CLIENT_ID_LEN {
        return Err(MqttError::InvalidClientId {
            reason: format!(
                "client ID exceeds {MAX_CLIENT_ID_LEN} chars (got {})",
                id.len()
            ),
        });
    }
    Ok(())
}

/// Validates an MQTT topic string.
///
/// Topics must not be empty and must not contain null characters.
fn validate_topic(topic: &str) -> Result<(), MqttError> {
    if topic.is_empty() {
        return Err(MqttError::InvalidTopic {
            reason: "topic cannot be empty".to_string(),
        });
    }
    if topic.contains('\0') {
        return Err(MqttError::InvalidTopic {
            reason: "topic cannot contain null character".to_string(),
        });
    }
    Ok(())
}

/// Validates an MQTT topic filter (subscription pattern).
///
/// - `+` matches a single level
/// - `#` matches all remaining levels (must be last character after `/`)
fn validate_topic_filter(filter: &str) -> Result<(), MqttError> {
    validate_topic(filter)?;

    let parts: Vec<&str> = filter.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        // `+` must occupy entire level
        if part.contains('+') && *part != "+" {
            return Err(MqttError::InvalidTopic {
                reason: format!("'+' wildcard must occupy entire level, got '{part}'"),
            });
        }
        // `#` must be the last level and alone
        if part.contains('#') {
            if *part != "#" {
                return Err(MqttError::InvalidTopic {
                    reason: format!("'#' wildcard must occupy entire level, got '{part}'"),
                });
            }
            if i != parts.len() - 1 {
                return Err(MqttError::InvalidTopic {
                    reason: "'#' wildcard must be the last level".to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Checks whether a topic name matches a topic filter.
///
/// Supports `+` (single-level) and `#` (multi-level) wildcards.
pub fn topic_matches(filter: &str, topic: &str) -> bool {
    let filter_parts: Vec<&str> = filter.split('/').collect();
    let topic_parts: Vec<&str> = topic.split('/').collect();

    let mut fi = 0;
    let mut ti = 0;

    while fi < filter_parts.len() && ti < topic_parts.len() {
        if filter_parts[fi] == "#" {
            return true; // # matches everything from here
        }
        if filter_parts[fi] != "+" && filter_parts[fi] != topic_parts[ti] {
            return false;
        }
        fi += 1;
        ti += 1;
    }

    // Both must be exhausted (unless filter ended with #)
    fi == filter_parts.len() && ti == topic_parts.len()
}

// ═══════════════════════════════════════════════════════════════════════
// MQTT client (simulation)
// ═══════════════════════════════════════════════════════════════════════

/// Callback type for received messages.
pub type MessageCallback = Box<dyn Fn(&MqttMessage) + Send>;

/// Simulated MQTT client.
///
/// Models MQTT 3.1.1 protocol operations:
/// `connect()` -> `subscribe()` / `publish()` -> `disconnect()`.
pub struct MqttClient {
    /// Client configuration.
    config: MqttConfig,
    /// Whether the client is connected.
    connected: bool,
    /// Active topic subscriptions.
    subscriptions: Vec<(String, QoS)>,
    /// Registered message callback.
    message_callback: Option<MessageCallback>,
    /// Published messages log (for testing).
    published: Vec<MqttMessage>,
    /// Next message ID for QoS 1.
    next_message_id: u16,
    /// Current reconnect delay (exponential backoff).
    reconnect_delay_ms: u64,
}

// Custom Debug because MessageCallback is not Debug
impl std::fmt::Debug for MqttClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MqttClient")
            .field("config", &self.config)
            .field("connected", &self.connected)
            .field("subscriptions", &self.subscriptions)
            .field("has_callback", &self.message_callback.is_some())
            .field("published_count", &self.published.len())
            .finish()
    }
}

impl MqttClient {
    /// Connects to the MQTT broker (simulation).
    ///
    /// Validates the configuration and simulates a successful connection.
    pub fn connect(config: MqttConfig) -> Result<Self, MqttError> {
        config.validate()?;
        Ok(Self {
            config,
            connected: true,
            subscriptions: Vec::new(),
            message_callback: None,
            published: Vec::new(),
            next_message_id: 1,
            reconnect_delay_ms: RECONNECT_INITIAL_MS,
        })
    }

    /// Returns whether the client is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Returns the client configuration.
    pub fn config(&self) -> &MqttConfig {
        &self.config
    }

    /// Returns active subscriptions.
    pub fn subscriptions(&self) -> &[(String, QoS)] {
        &self.subscriptions
    }

    /// Returns the log of published messages (for testing).
    pub fn published_messages(&self) -> &[MqttMessage] {
        &self.published
    }

    /// Publishes a message to a topic (simulation).
    pub fn publish(&mut self, topic: &str, payload: &[u8], qos: QoS) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }
        validate_topic(topic)?;
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(MqttError::PayloadTooLarge {
                size: payload.len(),
                max: MAX_PAYLOAD_SIZE,
            });
        }

        let msg_id = match qos {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce => {
                let id = self.next_message_id;
                self.next_message_id = self.next_message_id.wrapping_add(1);
                if self.next_message_id == 0 {
                    self.next_message_id = 1;
                }
                id
            }
        };

        let msg = MqttMessage {
            topic: topic.to_string(),
            payload: payload.to_vec(),
            qos,
            retain: false,
            message_id: msg_id,
        };

        self.published.push(msg);
        Ok(())
    }

    /// Subscribes to a topic filter (simulation).
    ///
    /// Supports MQTT wildcards: `+` (single level) and `#` (multi-level).
    pub fn subscribe(&mut self, topic_filter: &str, qos: QoS) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }
        validate_topic_filter(topic_filter)?;

        // Avoid duplicate subscriptions
        let already_subscribed = self.subscriptions.iter().any(|(t, _)| t == topic_filter);
        if !already_subscribed {
            self.subscriptions.push((topic_filter.to_string(), qos));
        }
        Ok(())
    }

    /// Unsubscribes from a topic filter.
    pub fn unsubscribe(&mut self, topic_filter: &str) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::NotConnected);
        }
        self.subscriptions.retain(|(t, _)| t != topic_filter);
        Ok(())
    }

    /// Registers a callback for received messages.
    pub fn on_message<F>(&mut self, callback: F)
    where
        F: Fn(&MqttMessage) + Send + 'static,
    {
        self.message_callback = Some(Box::new(callback));
    }

    /// Simulates receiving a message (for testing).
    ///
    /// Checks if any subscription matches and invokes the callback.
    pub fn simulate_receive(&self, msg: &MqttMessage) -> bool {
        let matched = self
            .subscriptions
            .iter()
            .any(|(filter, _)| topic_matches(filter, &msg.topic));

        if matched {
            if let Some(ref cb) = self.message_callback {
                cb(msg);
            }
        }
        matched
    }

    /// Disconnects from the broker (simulation).
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.subscriptions.clear();
    }

    /// Simulates a reconnection attempt with exponential backoff.
    ///
    /// Returns the delay in milliseconds before the next retry.
    pub fn reconnect(&mut self) -> Result<u64, MqttError> {
        if self.connected {
            return Err(MqttError::AlreadyConnected {
                broker_url: self.config.broker_url.clone(),
            });
        }

        let delay = self.reconnect_delay_ms;

        // Simulate successful reconnection
        self.connected = true;
        // Reset backoff on success
        self.reconnect_delay_ms = self.config.reconnect_initial_ms;

        Ok(delay)
    }

    /// Calculates the next exponential backoff delay.
    ///
    /// Doubles the current delay up to the maximum.
    pub fn next_backoff_delay(&mut self) -> u64 {
        let delay = self.reconnect_delay_ms;
        self.reconnect_delay_ms = (self.reconnect_delay_ms * 2).min(self.config.reconnect_max_ms);
        delay
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn mqtt_connect_succeeds() {
        let config = MqttConfig::new("mqtt://localhost:1883", "fj-test-1");
        let client = MqttClient::connect(config).unwrap();
        assert!(client.is_connected());
        assert_eq!(client.subscriptions().len(), 0);
    }

    #[test]
    fn mqtt_connect_invalid_url_fails() {
        let config = MqttConfig::new("http://wrong", "fj-test");
        let err = MqttClient::connect(config).unwrap_err();
        assert!(matches!(err, MqttError::InvalidBrokerUrl { .. }));

        let config = MqttConfig::new("", "fj-test");
        let err = MqttClient::connect(config).unwrap_err();
        assert!(matches!(err, MqttError::InvalidBrokerUrl { .. }));
    }

    #[test]
    fn mqtt_connect_invalid_client_id_fails() {
        let config = MqttConfig::new("mqtt://localhost", "");
        let err = MqttClient::connect(config).unwrap_err();
        assert!(matches!(err, MqttError::InvalidClientId { .. }));

        let long_id = "a".repeat(24);
        let config = MqttConfig::new("mqtt://localhost", &long_id);
        let err = MqttClient::connect(config).unwrap_err();
        assert!(matches!(err, MqttError::InvalidClientId { .. }));
    }

    #[test]
    fn mqtt_publish_qos0() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();

        client
            .publish("sensor/temp", b"25.3", QoS::AtMostOnce)
            .unwrap();

        assert_eq!(client.published_messages().len(), 1);
        let msg = &client.published_messages()[0];
        assert_eq!(msg.topic, "sensor/temp");
        assert_eq!(msg.payload, b"25.3");
        assert_eq!(msg.qos, QoS::AtMostOnce);
        assert_eq!(msg.message_id, 0);
    }

    #[test]
    fn mqtt_publish_qos1_assigns_message_id() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();

        client
            .publish("sensor/temp", b"25.3", QoS::AtLeastOnce)
            .unwrap();
        client
            .publish("sensor/hum", b"60", QoS::AtLeastOnce)
            .unwrap();

        assert_eq!(client.published_messages()[0].message_id, 1);
        assert_eq!(client.published_messages()[1].message_id, 2);
    }

    #[test]
    fn mqtt_subscribe_with_wildcards() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();

        client
            .subscribe("sensor/+/temperature", QoS::AtLeastOnce)
            .unwrap();
        client.subscribe("alerts/#", QoS::AtMostOnce).unwrap();

        assert_eq!(client.subscriptions().len(), 2);
        assert_eq!(client.subscriptions()[0].0, "sensor/+/temperature");
        assert_eq!(client.subscriptions()[1].0, "alerts/#");
    }

    #[test]
    fn mqtt_subscribe_invalid_filter() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();

        // `+` must occupy entire level
        let err = client
            .subscribe("sensor/temp+", QoS::AtMostOnce)
            .unwrap_err();
        assert!(matches!(err, MqttError::InvalidTopic { .. }));

        // `#` must be last level
        let err = client.subscribe("#/sensor", QoS::AtMostOnce).unwrap_err();
        assert!(matches!(err, MqttError::InvalidTopic { .. }));
    }

    #[test]
    fn mqtt_topic_matching() {
        // Exact match
        assert!(topic_matches("sensor/temp", "sensor/temp"));
        assert!(!topic_matches("sensor/temp", "sensor/hum"));

        // + wildcard (single level)
        assert!(topic_matches("sensor/+/temp", "sensor/1/temp"));
        assert!(topic_matches("sensor/+/temp", "sensor/abc/temp"));
        assert!(!topic_matches("sensor/+/temp", "sensor/1/2/temp"));

        // # wildcard (multi-level)
        assert!(topic_matches("sensor/#", "sensor/temp"));
        assert!(topic_matches("sensor/#", "sensor/1/temp"));
        assert!(topic_matches("sensor/#", "sensor/1/2/3"));
        assert!(!topic_matches("sensor/#", "device/1"));
    }

    #[test]
    fn mqtt_on_message_callback() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        client.on_message(move |msg| {
            received_clone.lock().unwrap().push(msg.topic.clone());
        });

        client.subscribe("sensor/#", QoS::AtMostOnce).unwrap();

        let msg = MqttMessage::new("sensor/temp", b"25.3", QoS::AtMostOnce);
        let matched = client.simulate_receive(&msg);
        assert!(matched);

        let received_topics = received.lock().unwrap();
        assert_eq!(received_topics.len(), 1);
        assert_eq!(received_topics[0], "sensor/temp");
    }

    #[test]
    fn mqtt_disconnect_clears_subscriptions() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1");
        let mut client = MqttClient::connect(config).unwrap();
        client.subscribe("test/#", QoS::AtMostOnce).unwrap();
        assert_eq!(client.subscriptions().len(), 1);

        client.disconnect();
        assert!(!client.is_connected());
        assert_eq!(client.subscriptions().len(), 0);
    }

    #[test]
    fn mqtt_exponential_backoff() {
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1").with_reconnect(1000, 60_000);
        let mut client = MqttClient::connect(config).unwrap();

        client.disconnect();

        // First reconnect: delay = 1000
        let delay = client.reconnect().unwrap();
        assert_eq!(delay, 1000);

        // Disconnect again and check backoff progresses
        client.disconnect();
        client.next_backoff_delay(); // advances to 2000
        let delay = client.reconnect().unwrap();
        assert_eq!(delay, 2000);
    }

    #[test]
    fn mqtt_last_will_config() {
        let will = LastWill::new("status/fj-1", b"offline", QoS::AtLeastOnce, true);
        let config = MqttConfig::new("mqtt://localhost", "fj-test-1").with_last_will(will.clone());

        assert!(config.last_will.is_some());
        let w = config.last_will.unwrap();
        assert_eq!(w.topic, "status/fj-1");
        assert_eq!(w.message, b"offline");
        assert_eq!(w.qos, QoS::AtLeastOnce);
        assert!(w.retain);
    }
}
