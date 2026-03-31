//! Raft Consensus — leader election, log replication, commit/apply,
//! WAL persistence, snapshots, membership changes, pre-vote, lease reads.
//!
//! Sprint D1: Raft Consensus (10 tasks)
//! All simulated — no real networking.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D1.1: Raft Node State
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a Raft node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RaftNodeId(pub u64);

impl fmt::Display for RaftNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Raft({})", self.0)
    }
}

/// Raft node role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaftRole {
    /// Passive participant — replicates log from leader.
    Follower,
    /// Seeking election.
    Candidate,
    /// Active leader — handles client requests and replicates log.
    Leader,
}

impl fmt::Display for RaftRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RaftRole::Follower => write!(f, "Follower"),
            RaftRole::Candidate => write!(f, "Candidate"),
            RaftRole::Leader => write!(f, "Leader"),
        }
    }
}

/// A single entry in the Raft log.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// Term when entry was created.
    pub term: u64,
    /// Log index (1-based).
    pub index: u64,
    /// Command payload.
    pub command: Vec<u8>,
}

/// Core Raft node state.
#[derive(Debug)]
pub struct RaftNode {
    /// This node's ID.
    pub id: RaftNodeId,
    /// Current role.
    pub role: RaftRole,
    /// Current term (monotonically increasing).
    pub current_term: u64,
    /// Who this node voted for in the current term.
    pub voted_for: Option<RaftNodeId>,
    /// Log entries (append-only).
    pub log: Vec<LogEntry>,
    /// Index of highest log entry known to be committed.
    pub commit_index: u64,
    /// Index of highest log entry applied to state machine.
    pub last_applied: u64,
    /// Cluster peer IDs (not including self).
    pub peers: Vec<RaftNodeId>,
    /// Leader state: next index to send to each peer.
    pub next_index: HashMap<RaftNodeId, u64>,
    /// Leader state: highest index known replicated on each peer.
    pub match_index: HashMap<RaftNodeId, u64>,
    /// Votes received in current election.
    pub votes_received: Vec<RaftNodeId>,
    /// Leader lease expiry (simulated timestamp in ms).
    pub lease_expiry_ms: u64,
    /// Pre-vote support enabled.
    pub pre_vote_enabled: bool,
}

impl RaftNode {
    /// Creates a new Raft node as a follower.
    pub fn new(id: RaftNodeId, peers: Vec<RaftNodeId>) -> Self {
        RaftNode {
            id,
            role: RaftRole::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            peers,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            votes_received: Vec::new(),
            lease_expiry_ms: 0,
            pre_vote_enabled: false,
        }
    }

    /// Returns the last log index (0 if empty).
    pub fn last_log_index(&self) -> u64 {
        self.log.last().map_or(0, |e| e.index)
    }

    /// Returns the last log term (0 if empty).
    pub fn last_log_term(&self) -> u64 {
        self.log.last().map_or(0, |e| e.term)
    }

    /// Total cluster size (self + peers).
    pub fn cluster_size(&self) -> usize {
        self.peers.len() + 1
    }

    /// Majority quorum size.
    pub fn quorum(&self) -> usize {
        self.cluster_size() / 2 + 1
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.2: Leader Election (RequestVote)
// ═══════════════════════════════════════════════════════════════════════

/// RequestVote RPC arguments.
#[derive(Debug, Clone)]
pub struct RequestVoteArgs {
    /// Candidate's term.
    pub term: u64,
    /// Candidate requesting the vote.
    pub candidate_id: RaftNodeId,
    /// Index of candidate's last log entry.
    pub last_log_index: u64,
    /// Term of candidate's last log entry.
    pub last_log_term: u64,
    /// Whether this is a pre-vote (does not advance term).
    pub pre_vote: bool,
}

/// RequestVote RPC reply.
#[derive(Debug, Clone)]
pub struct RequestVoteReply {
    /// Current term (for candidate to update itself).
    pub term: u64,
    /// True if vote was granted.
    pub vote_granted: bool,
}

/// Starts an election: increments term, votes for self, transitions to Candidate.
pub fn start_election(node: &mut RaftNode) {
    node.current_term += 1;
    node.role = RaftRole::Candidate;
    node.voted_for = Some(node.id);
    node.votes_received = vec![node.id]; // vote for self
}

/// Builds a RequestVote request from the current node state.
pub fn build_request_vote(node: &RaftNode, pre_vote: bool) -> RequestVoteArgs {
    RequestVoteArgs {
        term: node.current_term,
        candidate_id: node.id,
        last_log_index: node.last_log_index(),
        last_log_term: node.last_log_term(),
        pre_vote,
    }
}

/// Handles a RequestVote RPC on a receiver node.
pub fn handle_request_vote(node: &mut RaftNode, args: &RequestVoteArgs) -> RequestVoteReply {
    // If the request term is behind, reject.
    if args.term < node.current_term {
        return RequestVoteReply {
            term: node.current_term,
            vote_granted: false,
        };
    }

    // If we see a higher term, step down.
    if !args.pre_vote && args.term > node.current_term {
        node.current_term = args.term;
        node.role = RaftRole::Follower;
        node.voted_for = None;
    }

    // Check if we can grant the vote.
    let can_vote = node.voted_for.is_none() || node.voted_for == Some(args.candidate_id);

    // Log completeness check: candidate's log must be at least as up-to-date.
    let log_ok = args.last_log_term > node.last_log_term()
        || (args.last_log_term == node.last_log_term()
            && args.last_log_index >= node.last_log_index());

    let granted = can_vote && log_ok;
    if granted && !args.pre_vote {
        node.voted_for = Some(args.candidate_id);
    }

    RequestVoteReply {
        term: node.current_term,
        vote_granted: granted,
    }
}

/// Processes a received vote. Returns true if the node has won the election.
pub fn receive_vote(node: &mut RaftNode, from: RaftNodeId, reply: &RequestVoteReply) -> bool {
    if reply.term > node.current_term {
        node.current_term = reply.term;
        node.role = RaftRole::Follower;
        node.voted_for = None;
        return false;
    }

    if reply.vote_granted && node.role == RaftRole::Candidate {
        if !node.votes_received.contains(&from) {
            node.votes_received.push(from);
        }
        if node.votes_received.len() >= node.quorum() {
            become_leader(node);
            return true;
        }
    }
    false
}

/// Transitions a candidate to leader, initializing leader state.
pub fn become_leader(node: &mut RaftNode) {
    node.role = RaftRole::Leader;
    let next = node.last_log_index() + 1;
    for &peer in &node.peers {
        node.next_index.insert(peer, next);
        node.match_index.insert(peer, 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.3: Log Replication (AppendEntries)
// ═══════════════════════════════════════════════════════════════════════

/// AppendEntries RPC arguments.
#[derive(Debug, Clone)]
pub struct AppendEntriesArgs {
    /// Leader's term.
    pub term: u64,
    /// Leader's ID.
    pub leader_id: RaftNodeId,
    /// Index of log entry immediately preceding new ones.
    pub prev_log_index: u64,
    /// Term of prev_log_index entry.
    pub prev_log_term: u64,
    /// Log entries to store (empty for heartbeat).
    pub entries: Vec<LogEntry>,
    /// Leader's commit index.
    pub leader_commit: u64,
}

/// AppendEntries RPC reply.
#[derive(Debug, Clone)]
pub struct AppendEntriesReply {
    /// Current term (for leader to update itself).
    pub term: u64,
    /// True if follower contained entry matching prev_log_index/prev_log_term.
    pub success: bool,
    /// Hint for leader to fast-rewind next_index on conflict.
    pub conflict_index: Option<u64>,
}

/// Handles an AppendEntries RPC on a follower node.
pub fn handle_append_entries(node: &mut RaftNode, args: &AppendEntriesArgs) -> AppendEntriesReply {
    // Reject if leader's term is stale.
    if args.term < node.current_term {
        return AppendEntriesReply {
            term: node.current_term,
            success: false,
            conflict_index: None,
        };
    }

    // Step down if we see a higher or equal term.
    if args.term >= node.current_term {
        node.current_term = args.term;
        node.role = RaftRole::Follower;
        node.voted_for = None;
    }

    // Check log consistency at prev_log_index.
    if args.prev_log_index > 0 {
        let prev_entry = node.log.iter().find(|e| e.index == args.prev_log_index);
        match prev_entry {
            None => {
                return AppendEntriesReply {
                    term: node.current_term,
                    success: false,
                    conflict_index: Some(node.last_log_index() + 1),
                };
            }
            Some(entry) => {
                if entry.term != args.prev_log_term {
                    // Conflict: remove this entry and all that follow.
                    node.log.retain(|e| e.index < args.prev_log_index);
                    return AppendEntriesReply {
                        term: node.current_term,
                        success: false,
                        conflict_index: Some(args.prev_log_index),
                    };
                }
            }
        }
    }

    // Append new entries (skip already existing ones).
    for entry in &args.entries {
        let existing = node.log.iter().find(|e| e.index == entry.index);
        match existing {
            Some(e) if e.term == entry.term => {
                // Already have this entry; skip.
            }
            _ => {
                // Remove conflicting entries at this index and beyond.
                node.log.retain(|e| e.index < entry.index);
                node.log.push(entry.clone());
            }
        }
    }

    // Advance commit index.
    if args.leader_commit > node.commit_index {
        node.commit_index = args.leader_commit.min(node.last_log_index());
    }

    AppendEntriesReply {
        term: node.current_term,
        success: true,
        conflict_index: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.4: Commit & Apply
// ═══════════════════════════════════════════════════════════════════════

/// On the leader, advances commit_index based on match_index quorum.
pub fn leader_advance_commit(node: &mut RaftNode) {
    if node.role != RaftRole::Leader {
        return;
    }

    let last = node.last_log_index();
    for n in (node.commit_index + 1)..=last {
        // Only commit entries from the current term (Raft safety).
        let entry_term = node.log.iter().find(|e| e.index == n).map(|e| e.term);
        if entry_term != Some(node.current_term) {
            continue;
        }
        // Count replicas (including self).
        let mut count = 1usize; // self
        for &peer in &node.peers {
            if node.match_index.get(&peer).copied().unwrap_or(0) >= n {
                count += 1;
            }
        }
        if count >= node.quorum() {
            node.commit_index = n;
        }
    }
}

/// Applies committed but unapplied entries. Returns the commands applied.
pub fn apply_committed(node: &mut RaftNode) -> Vec<Vec<u8>> {
    let mut applied = Vec::new();
    while node.last_applied < node.commit_index {
        node.last_applied += 1;
        if let Some(entry) = node.log.iter().find(|e| e.index == node.last_applied) {
            applied.push(entry.command.clone());
        }
    }
    applied
}

// ═══════════════════════════════════════════════════════════════════════
// D1.5: Client Request (Leader Appends)
// ═══════════════════════════════════════════════════════════════════════

/// Appends a client command to the leader's log. Returns the entry or an error.
pub fn client_request(node: &mut RaftNode, command: Vec<u8>) -> Result<LogEntry, String> {
    if node.role != RaftRole::Leader {
        return Err("not the leader".to_string());
    }
    let entry = LogEntry {
        term: node.current_term,
        index: node.last_log_index() + 1,
        command,
    };
    node.log.push(entry.clone());
    Ok(entry)
}

// ═══════════════════════════════════════════════════════════════════════
// D1.6: WAL Persistence
// ═══════════════════════════════════════════════════════════════════════

/// Simulated WAL (Write-Ahead Log) persistence record.
#[derive(Debug, Clone)]
pub struct WalRecord {
    /// Term number.
    pub term: u64,
    /// Voted-for in this term.
    pub voted_for: Option<RaftNodeId>,
    /// Log entries.
    pub entries: Vec<LogEntry>,
}

/// Simulated WAL storage.
#[derive(Debug, Default)]
pub struct WalStorage {
    /// Persisted records.
    records: Vec<WalRecord>,
}

impl WalStorage {
    /// Creates a new empty WAL.
    pub fn new() -> Self {
        WalStorage::default()
    }

    /// Persists the current node state to WAL.
    pub fn persist(&mut self, node: &RaftNode) {
        self.records.push(WalRecord {
            term: node.current_term,
            voted_for: node.voted_for,
            entries: node.log.clone(),
        });
    }

    /// Recovers the latest persisted state. Returns None if WAL is empty.
    pub fn recover(&self) -> Option<&WalRecord> {
        self.records.last()
    }

    /// Returns the number of WAL records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Restores node state from WAL.
    pub fn restore(&self, node: &mut RaftNode) -> bool {
        if let Some(record) = self.recover() {
            node.current_term = record.term;
            node.voted_for = record.voted_for;
            node.log = record.entries.clone();
            true
        } else {
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.7: Snapshots
// ═══════════════════════════════════════════════════════════════════════

/// A snapshot of the state machine at a given index.
#[derive(Debug, Clone)]
pub struct RaftSnapshot {
    /// Last included index.
    pub last_included_index: u64,
    /// Last included term.
    pub last_included_term: u64,
    /// Serialized state machine data.
    pub data: Vec<u8>,
}

/// Creates a snapshot from the current node state and truncates the log.
pub fn create_snapshot(node: &mut RaftNode, state_data: Vec<u8>) -> Option<RaftSnapshot> {
    if node.last_applied == 0 {
        return None;
    }

    let last_term = node
        .log
        .iter()
        .find(|e| e.index == node.last_applied)
        .map(|e| e.term)
        .unwrap_or(0);

    let snapshot = RaftSnapshot {
        last_included_index: node.last_applied,
        last_included_term: last_term,
        data: state_data,
    };

    // Truncate log: remove entries up to and including last_applied.
    node.log.retain(|e| e.index > node.last_applied);

    Some(snapshot)
}

/// Restores from a snapshot (used when follower is far behind).
pub fn install_snapshot(node: &mut RaftNode, snapshot: &RaftSnapshot) {
    // Discard log entries covered by the snapshot.
    node.log.retain(|e| e.index > snapshot.last_included_index);

    if node.commit_index < snapshot.last_included_index {
        node.commit_index = snapshot.last_included_index;
    }
    if node.last_applied < snapshot.last_included_index {
        node.last_applied = snapshot.last_included_index;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.8: Membership Change (Joint Consensus)
// ═══════════════════════════════════════════════════════════════════════

/// A membership configuration for the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterConfig {
    /// Current voting members.
    pub members: Vec<RaftNodeId>,
}

/// Joint consensus configuration during membership change.
#[derive(Debug, Clone)]
pub struct JointConfig {
    /// Old configuration.
    pub old: ClusterConfig,
    /// New (proposed) configuration.
    pub new: ClusterConfig,
    /// Whether the joint phase has been committed.
    pub joint_committed: bool,
}

impl JointConfig {
    /// Creates a new joint configuration.
    pub fn new(old: ClusterConfig, new: ClusterConfig) -> Self {
        JointConfig {
            old,
            new,
            joint_committed: false,
        }
    }

    /// Checks if a quorum is reached in BOTH old and new configurations.
    pub fn has_joint_quorum(&self, voters: &[RaftNodeId]) -> bool {
        let old_quorum = self.old.members.len() / 2 + 1;
        let new_quorum = self.new.members.len() / 2 + 1;

        let old_count = voters
            .iter()
            .filter(|v| self.old.members.contains(v))
            .count();
        let new_count = voters
            .iter()
            .filter(|v| self.new.members.contains(v))
            .count();

        old_count >= old_quorum && new_count >= new_quorum
    }

    /// Marks the joint configuration as committed.
    pub fn commit_joint(&mut self) {
        self.joint_committed = true;
    }

    /// Finalizes the configuration change (transitions to new config only).
    pub fn finalize(&self) -> ClusterConfig {
        self.new.clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.9: Pre-Vote Extension
// ═══════════════════════════════════════════════════════════════════════

/// Pre-vote request: checks if an election would succeed without
/// incrementing the term. Prevents disruptions from partitioned nodes.
pub fn pre_vote_check(node: &RaftNode, args: &RequestVoteArgs) -> RequestVoteReply {
    // Pre-vote uses the same log completeness check but does NOT
    // change the receiver's state.
    let log_ok = args.last_log_term > node.last_log_term()
        || (args.last_log_term == node.last_log_term()
            && args.last_log_index >= node.last_log_index());

    // In pre-vote, grant if the candidate's term is at least as high
    // and its log is up-to-date.
    let granted = args.term >= node.current_term && log_ok;

    RequestVoteReply {
        term: node.current_term,
        vote_granted: granted,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.10: Lease-Based Reads
// ═══════════════════════════════════════════════════════════════════════

/// Checks if the leader's lease is still valid for serving reads.
pub fn lease_read_allowed(node: &RaftNode, now_ms: u64) -> bool {
    node.role == RaftRole::Leader && now_ms < node.lease_expiry_ms
}

/// Extends the leader's lease after receiving heartbeat acknowledgments
/// from a quorum.
pub fn extend_lease(node: &mut RaftNode, lease_duration_ms: u64, now_ms: u64) {
    if node.role == RaftRole::Leader {
        node.lease_expiry_ms = now_ms + lease_duration_ms;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peers(ids: &[u64]) -> Vec<RaftNodeId> {
        ids.iter().map(|&id| RaftNodeId(id)).collect()
    }

    fn make_node(id: u64, peers: &[u64]) -> RaftNode {
        RaftNode::new(RaftNodeId(id), make_peers(peers))
    }

    // D1.1 — Raft Node State
    #[test]
    fn d1_1_new_node_is_follower() {
        let node = make_node(1, &[2, 3]);
        assert_eq!(node.role, RaftRole::Follower);
        assert_eq!(node.current_term, 0);
        assert_eq!(node.last_log_index(), 0);
        assert_eq!(node.cluster_size(), 3);
        assert_eq!(node.quorum(), 2);
    }

    #[test]
    fn d1_1_node_id_display() {
        assert_eq!(RaftNodeId(42).to_string(), "Raft(42)");
    }

    #[test]
    fn d1_1_role_display() {
        assert_eq!(RaftRole::Leader.to_string(), "Leader");
        assert_eq!(RaftRole::Follower.to_string(), "Follower");
        assert_eq!(RaftRole::Candidate.to_string(), "Candidate");
    }

    // D1.2 — Leader Election
    #[test]
    fn d1_2_start_election() {
        let mut node = make_node(1, &[2, 3]);
        start_election(&mut node);
        assert_eq!(node.role, RaftRole::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some(RaftNodeId(1)));
        assert_eq!(node.votes_received.len(), 1); // voted for self
    }

    #[test]
    fn d1_2_handle_request_vote_grants() {
        let mut node = make_node(2, &[1, 3]);
        let args = RequestVoteArgs {
            term: 1,
            candidate_id: RaftNodeId(1),
            last_log_index: 0,
            last_log_term: 0,
            pre_vote: false,
        };
        let reply = handle_request_vote(&mut node, &args);
        assert!(reply.vote_granted);
        assert_eq!(node.voted_for, Some(RaftNodeId(1)));
    }

    #[test]
    fn d1_2_handle_request_vote_rejects_stale_term() {
        let mut node = make_node(2, &[1, 3]);
        node.current_term = 5;
        let args = RequestVoteArgs {
            term: 3,
            candidate_id: RaftNodeId(1),
            last_log_index: 0,
            last_log_term: 0,
            pre_vote: false,
        };
        let reply = handle_request_vote(&mut node, &args);
        assert!(!reply.vote_granted);
    }

    #[test]
    fn d1_2_win_election_with_quorum() {
        let mut node = make_node(1, &[2, 3, 4, 5]);
        start_election(&mut node);
        // Need 3 votes (quorum of 5 = 3). Already have 1 (self).
        let reply_yes = RequestVoteReply {
            term: 1,
            vote_granted: true,
        };
        assert!(!receive_vote(&mut node, RaftNodeId(2), &reply_yes));
        assert!(receive_vote(&mut node, RaftNodeId(3), &reply_yes)); // 3 votes = quorum
        assert_eq!(node.role, RaftRole::Leader);
    }

    // D1.3 — Log Replication
    #[test]
    fn d1_3_append_entries_heartbeat() {
        let mut follower = make_node(2, &[1, 3]);
        let args = AppendEntriesArgs {
            term: 1,
            leader_id: RaftNodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };
        let reply = handle_append_entries(&mut follower, &args);
        assert!(reply.success);
        assert_eq!(follower.current_term, 1);
    }

    #[test]
    fn d1_3_append_entries_with_data() {
        let mut follower = make_node(2, &[1, 3]);
        let entries = vec![
            LogEntry {
                term: 1,
                index: 1,
                command: b"set x=1".to_vec(),
            },
            LogEntry {
                term: 1,
                index: 2,
                command: b"set y=2".to_vec(),
            },
        ];
        let args = AppendEntriesArgs {
            term: 1,
            leader_id: RaftNodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries,
            leader_commit: 1,
        };
        let reply = handle_append_entries(&mut follower, &args);
        assert!(reply.success);
        assert_eq!(follower.log.len(), 2);
        assert_eq!(follower.commit_index, 1);
    }

    #[test]
    fn d1_3_append_entries_rejects_stale_term() {
        let mut follower = make_node(2, &[1, 3]);
        follower.current_term = 5;
        let args = AppendEntriesArgs {
            term: 3,
            leader_id: RaftNodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };
        let reply = handle_append_entries(&mut follower, &args);
        assert!(!reply.success);
    }

    // D1.4 — Commit & Apply
    #[test]
    fn d1_4_leader_advance_commit() {
        let mut leader = make_node(1, &[2, 3]);
        become_leader(&mut leader);
        leader.current_term = 1;
        leader.log.push(LogEntry {
            term: 1,
            index: 1,
            command: b"cmd1".to_vec(),
        });
        // Peer 2 has replicated index 1.
        leader.match_index.insert(RaftNodeId(2), 1);
        leader_advance_commit(&mut leader);
        assert_eq!(leader.commit_index, 1); // Quorum: self + peer2 = 2 >= 2
    }

    #[test]
    fn d1_4_apply_committed_entries() {
        let mut node = make_node(1, &[2, 3]);
        node.log.push(LogEntry {
            term: 1,
            index: 1,
            command: b"a".to_vec(),
        });
        node.log.push(LogEntry {
            term: 1,
            index: 2,
            command: b"b".to_vec(),
        });
        node.commit_index = 2;
        let applied = apply_committed(&mut node);
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0], b"a");
        assert_eq!(applied[1], b"b");
        assert_eq!(node.last_applied, 2);
    }

    // D1.5 — Client Request
    #[test]
    fn d1_5_client_request_on_leader() {
        let mut leader = make_node(1, &[2, 3]);
        become_leader(&mut leader);
        leader.current_term = 1;
        let entry = client_request(&mut leader, b"set z=3".to_vec()).unwrap();
        assert_eq!(entry.term, 1);
        assert_eq!(entry.index, 1);
        assert_eq!(leader.log.len(), 1);
    }

    #[test]
    fn d1_5_client_request_on_follower_fails() {
        let mut follower = make_node(2, &[1, 3]);
        let result = client_request(&mut follower, b"cmd".to_vec());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "not the leader");
    }

    // D1.6 — WAL Persistence
    #[test]
    fn d1_6_wal_persist_and_recover() {
        let mut node = make_node(1, &[2, 3]);
        node.current_term = 3;
        node.voted_for = Some(RaftNodeId(2));
        node.log.push(LogEntry {
            term: 3,
            index: 1,
            command: b"x".to_vec(),
        });

        let mut wal = WalStorage::new();
        wal.persist(&node);
        assert_eq!(wal.record_count(), 1);

        let record = wal.recover().unwrap();
        assert_eq!(record.term, 3);
        assert_eq!(record.voted_for, Some(RaftNodeId(2)));
        assert_eq!(record.entries.len(), 1);
    }

    #[test]
    fn d1_6_wal_restore() {
        let mut original = make_node(1, &[2, 3]);
        original.current_term = 5;
        original.voted_for = Some(RaftNodeId(3));
        original.log.push(LogEntry {
            term: 5,
            index: 1,
            command: b"cmd".to_vec(),
        });

        let mut wal = WalStorage::new();
        wal.persist(&original);

        let mut restored = make_node(1, &[2, 3]);
        assert!(wal.restore(&mut restored));
        assert_eq!(restored.current_term, 5);
        assert_eq!(restored.voted_for, Some(RaftNodeId(3)));
        assert_eq!(restored.log.len(), 1);
    }

    // D1.7 — Snapshots
    #[test]
    fn d1_7_create_snapshot() {
        let mut node = make_node(1, &[2, 3]);
        node.log.push(LogEntry {
            term: 1,
            index: 1,
            command: b"a".to_vec(),
        });
        node.log.push(LogEntry {
            term: 1,
            index: 2,
            command: b"b".to_vec(),
        });
        node.log.push(LogEntry {
            term: 2,
            index: 3,
            command: b"c".to_vec(),
        });
        node.last_applied = 2;

        let snap = create_snapshot(&mut node, b"state-data".to_vec()).unwrap();
        assert_eq!(snap.last_included_index, 2);
        assert_eq!(snap.last_included_term, 1);
        // Log should be truncated: only entry 3 remains.
        assert_eq!(node.log.len(), 1);
        assert_eq!(node.log[0].index, 3);
    }

    #[test]
    fn d1_7_install_snapshot() {
        let mut node = make_node(2, &[1, 3]);
        let snap = RaftSnapshot {
            last_included_index: 10,
            last_included_term: 3,
            data: b"snapshot".to_vec(),
        };
        install_snapshot(&mut node, &snap);
        assert_eq!(node.commit_index, 10);
        assert_eq!(node.last_applied, 10);
    }

    // D1.8 — Membership Change (Joint Consensus)
    #[test]
    fn d1_8_joint_quorum() {
        let old = ClusterConfig {
            members: vec![RaftNodeId(1), RaftNodeId(2), RaftNodeId(3)],
        };
        let new = ClusterConfig {
            members: vec![RaftNodeId(1), RaftNodeId(2), RaftNodeId(4)],
        };
        let joint = JointConfig::new(old, new);

        // Voters: 1, 2 — quorum in old (2/3) and new (2/3).
        assert!(joint.has_joint_quorum(&[RaftNodeId(1), RaftNodeId(2)]));
        // Voters: 3, 4 — quorum in old (1/3 fail) or new (1/3 fail).
        assert!(!joint.has_joint_quorum(&[RaftNodeId(3), RaftNodeId(4)]));
    }

    #[test]
    fn d1_8_joint_finalize() {
        let old = ClusterConfig {
            members: vec![RaftNodeId(1), RaftNodeId(2), RaftNodeId(3)],
        };
        let new = ClusterConfig {
            members: vec![RaftNodeId(1), RaftNodeId(4), RaftNodeId(5)],
        };
        let mut joint = JointConfig::new(old, new.clone());
        joint.commit_joint();
        assert!(joint.joint_committed);
        let final_config = joint.finalize();
        assert_eq!(final_config, new);
    }

    // D1.9 — Pre-Vote
    #[test]
    fn d1_9_pre_vote_grants() {
        let node = make_node(2, &[1, 3]);
        let args = RequestVoteArgs {
            term: 1,
            candidate_id: RaftNodeId(1),
            last_log_index: 0,
            last_log_term: 0,
            pre_vote: true,
        };
        let reply = pre_vote_check(&node, &args);
        assert!(reply.vote_granted);
    }

    #[test]
    fn d1_9_pre_vote_rejects_stale() {
        let mut node = make_node(2, &[1, 3]);
        node.current_term = 5;
        let args = RequestVoteArgs {
            term: 3,
            candidate_id: RaftNodeId(1),
            last_log_index: 0,
            last_log_term: 0,
            pre_vote: true,
        };
        let reply = pre_vote_check(&node, &args);
        assert!(!reply.vote_granted);
    }

    // D1.10 — Lease-Based Reads
    #[test]
    fn d1_10_lease_read() {
        let mut leader = make_node(1, &[2, 3]);
        become_leader(&mut leader);
        leader.current_term = 1;

        // No lease yet.
        assert!(!lease_read_allowed(&leader, 1000));

        // Extend lease.
        extend_lease(&mut leader, 5000, 1000);
        assert!(lease_read_allowed(&leader, 3000)); // within lease
        assert!(!lease_read_allowed(&leader, 7000)); // lease expired
    }

    #[test]
    fn d1_10_lease_not_granted_to_follower() {
        let mut node = make_node(2, &[1, 3]);
        extend_lease(&mut node, 5000, 1000); // should be no-op
        assert!(!lease_read_allowed(&node, 2000));
    }

    // Integration: full election + replication cycle
    #[test]
    fn d1_integration_election_and_replication() {
        // 3-node cluster: node 1 becomes leader, replicates to node 2.
        let mut n1 = make_node(1, &[2, 3]);
        let mut n2 = make_node(2, &[1, 3]);

        // Node 1 starts election.
        start_election(&mut n1);
        let rv_args = build_request_vote(&n1, false);

        // Node 2 votes for node 1.
        let rv_reply = handle_request_vote(&mut n2, &rv_args);
        assert!(rv_reply.vote_granted);
        let won = receive_vote(&mut n1, RaftNodeId(2), &rv_reply);
        assert!(won);
        assert_eq!(n1.role, RaftRole::Leader);

        // Leader appends a client request.
        let entry = client_request(&mut n1, b"set key=val".to_vec()).unwrap();
        assert_eq!(entry.index, 1);

        // Replicate to node 2.
        let ae_args = AppendEntriesArgs {
            term: n1.current_term,
            leader_id: n1.id,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: n1.log.clone(),
            leader_commit: n1.commit_index,
        };
        let ae_reply = handle_append_entries(&mut n2, &ae_args);
        assert!(ae_reply.success);
        assert_eq!(n2.log.len(), 1);

        // Leader records replication success.
        n1.match_index.insert(RaftNodeId(2), 1);
        leader_advance_commit(&mut n1);
        assert_eq!(n1.commit_index, 1);

        // Apply on leader.
        let applied = apply_committed(&mut n1);
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0], b"set key=val");
    }
}
