//! Actor Model — actor trait, spawn, mailbox, supervision,
//! registry, ask pattern, state persistence.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S7.1: Actor Trait
// ═══════════════════════════════════════════════════════════════════════

/// Action returned by an actor after handling a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorAction {
    /// Continue processing messages.
    Continue,
    /// Stop the actor.
    Stop,
    /// Restart the actor (clear state, call `started()` again).
    Restart,
}

impl fmt::Display for ActorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorAction::Continue => write!(f, "Continue"),
            ActorAction::Stop => write!(f, "Stop"),
            ActorAction::Restart => write!(f, "Restart"),
        }
    }
}

/// Definition of an actor's behavior.
#[derive(Debug, Clone)]
pub struct ActorDef {
    /// Actor type name.
    pub name: String,
    /// Message type name.
    pub message_type: String,
    /// Whether this actor is persistent.
    pub persistent: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// S7.2: Actor Spawn
// ═══════════════════════════════════════════════════════════════════════

/// Unique address for a spawned actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorAddr(pub u64);

impl fmt::Display for ActorAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorAddr({})", self.0)
    }
}

/// Status of an actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorStatus {
    /// Actor is starting up.
    Starting,
    /// Actor is running and processing messages.
    Running,
    /// Actor is stopping.
    Stopping,
    /// Actor has stopped.
    Stopped,
    /// Actor has failed and is waiting for supervision decision.
    Failed(String),
    /// Actor is restarting.
    Restarting,
}

impl fmt::Display for ActorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorStatus::Starting => write!(f, "Starting"),
            ActorStatus::Running => write!(f, "Running"),
            ActorStatus::Stopping => write!(f, "Stopping"),
            ActorStatus::Stopped => write!(f, "Stopped"),
            ActorStatus::Failed(e) => write!(f, "Failed({e})"),
            ActorStatus::Restarting => write!(f, "Restarting"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.3-S7.4: Message Send & Mailbox
// ═══════════════════════════════════════════════════════════════════════

/// A message in an actor's mailbox.
#[derive(Debug, Clone)]
pub struct ActorMessage {
    /// Message content.
    pub content: String,
    /// Sender address (if any).
    pub sender: Option<ActorAddr>,
    /// Whether this is a request expecting a response.
    pub expects_reply: bool,
}

/// A bounded mailbox for an actor.
#[derive(Debug)]
pub struct Mailbox {
    /// Messages waiting to be processed.
    messages: Vec<ActorMessage>,
    /// Maximum mailbox capacity.
    pub capacity: usize,
}

impl Mailbox {
    /// Creates a new mailbox with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Mailbox {
            messages: Vec::new(),
            capacity,
        }
    }

    /// Sends a message to the mailbox, applying backpressure if full.
    pub fn send(&mut self, msg: ActorMessage) -> Result<(), MailboxError> {
        if self.messages.len() >= self.capacity {
            return Err(MailboxError::Full {
                capacity: self.capacity,
            });
        }
        self.messages.push(msg);
        Ok(())
    }

    /// Receives the next message (FIFO).
    pub fn receive(&mut self) -> Option<ActorMessage> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }

    /// Returns the number of pending messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns true if the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Returns true if the mailbox is full.
    pub fn is_full(&self) -> bool {
        self.messages.len() >= self.capacity
    }
}

/// Error when a mailbox operation fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxError {
    /// Mailbox is full, cannot accept more messages.
    Full { capacity: usize },
}

impl fmt::Display for MailboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MailboxError::Full { capacity } => {
                write!(f, "mailbox full (capacity: {capacity})")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.5: Actor Lifecycle
// ═══════════════════════════════════════════════════════════════════════

/// Lifecycle events for an actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// Actor has started.
    Started,
    /// Actor received a message.
    MessageReceived(String),
    /// Actor has stopped.
    Stopped,
    /// Actor is restarting.
    Restarting,
}

impl fmt::Display for LifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleEvent::Started => write!(f, "Started"),
            LifecycleEvent::MessageReceived(msg) => write!(f, "Message({msg})"),
            LifecycleEvent::Stopped => write!(f, "Stopped"),
            LifecycleEvent::Restarting => write!(f, "Restarting"),
        }
    }
}

/// A simulated actor instance with lifecycle tracking.
#[derive(Debug)]
pub struct ActorInstance {
    /// Actor address.
    pub addr: ActorAddr,
    /// Actor name.
    pub name: String,
    /// Current status.
    pub status: ActorStatus,
    /// Mailbox.
    pub mailbox: Mailbox,
    /// Lifecycle event log.
    pub events: Vec<LifecycleEvent>,
    /// Internal state (key-value pairs for simulation).
    pub state: HashMap<String, String>,
    /// Restart count.
    pub restart_count: u32,
}

impl ActorInstance {
    /// Creates a new actor instance.
    pub fn new(addr: ActorAddr, name: &str, mailbox_capacity: usize) -> Self {
        ActorInstance {
            addr,
            name: name.to_string(),
            status: ActorStatus::Starting,
            mailbox: Mailbox::new(mailbox_capacity),
            events: vec![LifecycleEvent::Started],
            state: HashMap::new(),
            restart_count: 0,
        }
    }

    /// Transitions to running state.
    pub fn start(&mut self) {
        self.status = ActorStatus::Running;
        self.events.push(LifecycleEvent::Started);
    }

    /// Processes the next message in the mailbox.
    pub fn process_next(&mut self) -> Option<ActorAction> {
        let msg = self.mailbox.receive()?;
        self.events
            .push(LifecycleEvent::MessageReceived(msg.content.clone()));

        // Simulate message handling: "stop" → Stop, "fail:X" → fail, else Continue
        if msg.content == "stop" {
            self.status = ActorStatus::Stopping;
            self.events.push(LifecycleEvent::Stopped);
            Some(ActorAction::Stop)
        } else if let Some(err_msg) = msg.content.strip_prefix("fail:") {
            self.status = ActorStatus::Failed(err_msg.to_string());
            Some(ActorAction::Restart)
        } else {
            self.state
                .insert("last_message".into(), msg.content.clone());
            Some(ActorAction::Continue)
        }
    }

    /// Restarts the actor, clearing state.
    pub fn restart(&mut self) {
        self.status = ActorStatus::Restarting;
        self.events.push(LifecycleEvent::Restarting);
        self.state.clear();
        self.restart_count += 1;
        self.status = ActorStatus::Running;
        self.events.push(LifecycleEvent::Started);
    }

    /// Stops the actor.
    pub fn stop(&mut self) {
        self.status = ActorStatus::Stopped;
        self.events.push(LifecycleEvent::Stopped);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.6: Supervision Strategy
// ═══════════════════════════════════════════════════════════════════════

/// Supervision strategy for handling actor failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionStrategy {
    /// Restart only the failed actor.
    OneForOne,
    /// Restart all sibling actors when one fails.
    AllForOne,
    /// Restart the failed actor and all actors started after it.
    RestForOne,
}

impl fmt::Display for SupervisionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupervisionStrategy::OneForOne => write!(f, "OneForOne"),
            SupervisionStrategy::AllForOne => write!(f, "AllForOne"),
            SupervisionStrategy::RestForOne => write!(f, "RestForOne"),
        }
    }
}

/// Determines which actors to restart based on the strategy.
pub fn apply_supervision(
    strategy: &SupervisionStrategy,
    failed_index: usize,
    total_actors: usize,
) -> Vec<usize> {
    match strategy {
        SupervisionStrategy::OneForOne => vec![failed_index],
        SupervisionStrategy::AllForOne => (0..total_actors).collect(),
        SupervisionStrategy::RestForOne => (failed_index..total_actors).collect(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.7: Actor Registry
// ═══════════════════════════════════════════════════════════════════════

/// A registry of named actors.
#[derive(Debug, Default)]
pub struct ActorRegistry {
    /// Map from name to actor address.
    names: HashMap<String, ActorAddr>,
}

impl ActorRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        ActorRegistry {
            names: HashMap::new(),
        }
    }

    /// Registers an actor with a name.
    pub fn register(&mut self, name: &str, addr: ActorAddr) -> Result<(), String> {
        if self.names.contains_key(name) {
            return Err(format!("actor name `{name}` already registered"));
        }
        self.names.insert(name.to_string(), addr);
        Ok(())
    }

    /// Looks up an actor by name.
    pub fn get(&self, name: &str) -> Option<ActorAddr> {
        self.names.get(name).copied()
    }

    /// Unregisters an actor by name.
    pub fn unregister(&mut self, name: &str) -> Option<ActorAddr> {
        self.names.remove(name)
    }

    /// Returns the number of registered actors.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Returns all registered names.
    pub fn names(&self) -> Vec<&str> {
        self.names.keys().map(|s| s.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.8: Request-Response (Ask Pattern)
// ═══════════════════════════════════════════════════════════════════════

/// A pending request awaiting a response.
#[derive(Debug, Clone)]
pub struct PendingRequest {
    /// Request ID.
    pub id: u64,
    /// The request message.
    pub request: String,
    /// The response (filled when received).
    pub response: Option<String>,
    /// Requesting actor address.
    pub from: ActorAddr,
}

/// An ask-pattern manager for request-response interactions.
#[derive(Debug, Default)]
pub struct AskManager {
    /// Pending requests.
    requests: Vec<PendingRequest>,
    /// Next request ID.
    next_id: u64,
}

impl AskManager {
    /// Creates a new ask manager.
    pub fn new() -> Self {
        AskManager {
            requests: Vec::new(),
            next_id: 1,
        }
    }

    /// Creates a request and returns the request ID.
    pub fn ask(&mut self, from: ActorAddr, request: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.requests.push(PendingRequest {
            id,
            request: request.to_string(),
            response: None,
            from,
        });
        id
    }

    /// Completes a request with a response.
    pub fn respond(&mut self, request_id: u64, response: &str) -> bool {
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == request_id) {
            req.response = Some(response.to_string());
            true
        } else {
            false
        }
    }

    /// Gets the response for a completed request.
    pub fn get_response(&self, request_id: u64) -> Option<&str> {
        self.requests
            .iter()
            .find(|r| r.id == request_id)
            .and_then(|r| r.response.as_deref())
    }

    /// Returns the number of pending (unanswered) requests.
    pub fn pending_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|r| r.response.is_none())
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S7.9: Actor State Persistence
// ═══════════════════════════════════════════════════════════════════════

/// A snapshot of actor state for persistence.
#[derive(Debug, Clone)]
pub struct ActorSnapshot {
    /// Actor name.
    pub actor_name: String,
    /// Serialized state.
    pub state: HashMap<String, String>,
    /// Snapshot version.
    pub version: u64,
}

/// A store for actor snapshots.
#[derive(Debug, Default)]
pub struct SnapshotStore {
    /// Snapshots indexed by actor name.
    snapshots: HashMap<String, ActorSnapshot>,
}

impl SnapshotStore {
    /// Creates a new empty store.
    pub fn new() -> Self {
        SnapshotStore {
            snapshots: HashMap::new(),
        }
    }

    /// Saves a snapshot.
    pub fn save(&mut self, snapshot: ActorSnapshot) {
        self.snapshots.insert(snapshot.actor_name.clone(), snapshot);
    }

    /// Loads a snapshot by actor name.
    pub fn load(&self, actor_name: &str) -> Option<&ActorSnapshot> {
        self.snapshots.get(actor_name)
    }

    /// Removes a snapshot.
    pub fn remove(&mut self, actor_name: &str) -> Option<ActorSnapshot> {
        self.snapshots.remove(actor_name)
    }

    /// Returns the number of stored snapshots.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns true if no snapshots are stored.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S7.1 — Actor Trait
    #[test]
    fn s7_1_actor_def() {
        let def = ActorDef {
            name: "Counter".into(),
            message_type: "CounterMsg".into(),
            persistent: false,
        };
        assert_eq!(def.name, "Counter");
        assert!(!def.persistent);
    }

    #[test]
    fn s7_1_actor_action_display() {
        assert_eq!(ActorAction::Continue.to_string(), "Continue");
        assert_eq!(ActorAction::Stop.to_string(), "Stop");
        assert_eq!(ActorAction::Restart.to_string(), "Restart");
    }

    // S7.2 — Actor Spawn
    #[test]
    fn s7_2_actor_spawn() {
        let actor = ActorInstance::new(ActorAddr(1), "worker", 1024);
        assert_eq!(actor.addr, ActorAddr(1));
        assert_eq!(actor.name, "worker");
        assert_eq!(actor.status, ActorStatus::Starting);
    }

    #[test]
    fn s7_2_actor_addr_display() {
        assert_eq!(ActorAddr(42).to_string(), "ActorAddr(42)");
    }

    // S7.3 — Message Send
    #[test]
    fn s7_3_send_message() {
        let mut actor = ActorInstance::new(ActorAddr(1), "worker", 1024);
        actor.start();
        actor
            .mailbox
            .send(ActorMessage {
                content: "hello".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        assert_eq!(actor.mailbox.len(), 1);
    }

    // S7.4 — Actor Mailbox
    #[test]
    fn s7_4_mailbox_backpressure() {
        let mut mailbox = Mailbox::new(2);
        mailbox
            .send(ActorMessage {
                content: "msg1".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        mailbox
            .send(ActorMessage {
                content: "msg2".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        let result = mailbox.send(ActorMessage {
            content: "msg3".into(),
            sender: None,
            expects_reply: false,
        });
        assert!(result.is_err());
        assert!(mailbox.is_full());
    }

    #[test]
    fn s7_4_mailbox_fifo() {
        let mut mailbox = Mailbox::new(10);
        mailbox
            .send(ActorMessage {
                content: "first".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        mailbox
            .send(ActorMessage {
                content: "second".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        assert_eq!(mailbox.receive().unwrap().content, "first");
        assert_eq!(mailbox.receive().unwrap().content, "second");
    }

    // S7.5 — Actor Lifecycle
    #[test]
    fn s7_5_lifecycle_events() {
        let mut actor = ActorInstance::new(ActorAddr(1), "worker", 1024);
        actor.start();
        actor
            .mailbox
            .send(ActorMessage {
                content: "hello".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        let action = actor.process_next().unwrap();
        assert_eq!(action, ActorAction::Continue);
        assert_eq!(actor.state.get("last_message").unwrap(), "hello");
    }

    #[test]
    fn s7_5_actor_stop() {
        let mut actor = ActorInstance::new(ActorAddr(1), "worker", 1024);
        actor.start();
        actor
            .mailbox
            .send(ActorMessage {
                content: "stop".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        let action = actor.process_next().unwrap();
        assert_eq!(action, ActorAction::Stop);
        assert_eq!(actor.status, ActorStatus::Stopping);
    }

    #[test]
    fn s7_5_actor_fail_restart() {
        let mut actor = ActorInstance::new(ActorAddr(1), "worker", 1024);
        actor.start();
        actor.state.insert("data".into(), "value".into());
        actor
            .mailbox
            .send(ActorMessage {
                content: "fail:crash".into(),
                sender: None,
                expects_reply: false,
            })
            .unwrap();
        let action = actor.process_next().unwrap();
        assert_eq!(action, ActorAction::Restart);
        actor.restart();
        assert_eq!(actor.restart_count, 1);
        assert!(actor.state.is_empty()); // State cleared on restart
    }

    // S7.6 — Supervision Strategy
    #[test]
    fn s7_6_one_for_one() {
        let indices = apply_supervision(&SupervisionStrategy::OneForOne, 2, 5);
        assert_eq!(indices, vec![2]);
    }

    #[test]
    fn s7_6_all_for_one() {
        let indices = apply_supervision(&SupervisionStrategy::AllForOne, 2, 5);
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn s7_6_rest_for_one() {
        let indices = apply_supervision(&SupervisionStrategy::RestForOne, 2, 5);
        assert_eq!(indices, vec![2, 3, 4]);
    }

    // S7.7 — Actor Registry
    #[test]
    fn s7_7_register_and_lookup() {
        let mut registry = ActorRegistry::new();
        registry.register("user_service", ActorAddr(1)).unwrap();
        registry.register("email_service", ActorAddr(2)).unwrap();

        assert_eq!(registry.get("user_service"), Some(ActorAddr(1)));
        assert_eq!(registry.get("unknown"), None);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn s7_7_duplicate_name_error() {
        let mut registry = ActorRegistry::new();
        registry.register("service", ActorAddr(1)).unwrap();
        let result = registry.register("service", ActorAddr(2));
        assert!(result.is_err());
    }

    #[test]
    fn s7_7_unregister() {
        let mut registry = ActorRegistry::new();
        registry.register("service", ActorAddr(1)).unwrap();
        let removed = registry.unregister("service");
        assert_eq!(removed, Some(ActorAddr(1)));
        assert!(registry.is_empty());
    }

    // S7.8 — Request-Response (Ask Pattern)
    #[test]
    fn s7_8_ask_and_respond() {
        let mut mgr = AskManager::new();
        let req_id = mgr.ask(ActorAddr(1), "get_user(42)");
        assert_eq!(mgr.pending_count(), 1);

        mgr.respond(req_id, "User { id: 42, name: Alice }");
        assert_eq!(mgr.pending_count(), 0);
        assert_eq!(
            mgr.get_response(req_id),
            Some("User { id: 42, name: Alice }")
        );
    }

    #[test]
    fn s7_8_respond_unknown_id() {
        let mut mgr = AskManager::new();
        assert!(!mgr.respond(999, "response"));
    }

    // S7.9 — Actor State Persistence
    #[test]
    fn s7_9_snapshot_save_load() {
        let mut store = SnapshotStore::new();
        let mut state = HashMap::new();
        state.insert("counter".into(), "42".into());
        store.save(ActorSnapshot {
            actor_name: "counter_actor".into(),
            state,
            version: 1,
        });

        let loaded = store.load("counter_actor").unwrap();
        assert_eq!(loaded.state.get("counter").unwrap(), "42");
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn s7_9_snapshot_remove() {
        let mut store = SnapshotStore::new();
        store.save(ActorSnapshot {
            actor_name: "actor1".into(),
            state: HashMap::new(),
            version: 1,
        });
        assert_eq!(store.len(), 1);
        store.remove("actor1");
        assert!(store.is_empty());
    }

    // S7.10 — Integration
    #[test]
    fn s7_10_supervision_display() {
        assert_eq!(SupervisionStrategy::OneForOne.to_string(), "OneForOne");
        assert_eq!(SupervisionStrategy::AllForOne.to_string(), "AllForOne");
        assert_eq!(SupervisionStrategy::RestForOne.to_string(), "RestForOne");
    }

    #[test]
    fn s7_10_lifecycle_event_display() {
        assert_eq!(LifecycleEvent::Started.to_string(), "Started");
        assert_eq!(
            LifecycleEvent::MessageReceived("hello".into()).to_string(),
            "Message(hello)"
        );
    }
}
