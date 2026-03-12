# Distributed Computing

Fajar Lang provides distributed computing primitives for building fault-tolerant, multi-node systems.

## Actor Model

```fajar
use distributed::actors

struct Counter {
    count: i64,
}

enum CounterMsg {
    Increment,
    Decrement,
    GetCount,
}

impl Actor for Counter {
    type Message = CounterMsg

    fn handle(&mut self, msg: CounterMsg) -> Option<i64> {
        match msg {
            Increment => { self.count += 1; None },
            Decrement => { self.count -= 1; None },
            GetCount => Some(self.count),
        }
    }
}

fn main() {
    let system = ActorSystem::new()
    let counter = system.spawn(Counter { count: 0 })

    counter.send(Increment)
    counter.send(Increment)
    counter.send(Increment)

    let count = counter.ask(GetCount)  // Returns 3
}
```

## Supervision

Actors are organized in supervision trees:

```fajar
let supervisor = Supervisor::new(Strategy::OneForOne)

supervisor.add_child("worker-1", WorkerActor::new())
supervisor.add_child("worker-2", WorkerActor::new())

// If worker-1 crashes, only worker-1 is restarted
// OneForAll would restart all children
// RestForOne restarts the failed child and those started after it
```

## Raft Consensus

```fajar
use distributed::consensus

let node = RaftNode::new(node_id: 1, peers: [2, 3, 4, 5])

// Leader election happens automatically
// Log replication is transparent

node.propose("set x = 42")  // Replicated to majority before commit
let value = node.read("x")  // Linearizable read
```

## CRDTs

Conflict-free Replicated Data Types for eventual consistency:

```fajar
use distributed::crdt

// G-Counter — grow-only counter (no coordination needed)
let counter = GCounter::new(node_id: 1)
counter.increment()
counter.increment()

// Merge with remote replica
counter.merge(remote_counter)
let total = counter.value()  // Correct across all replicas

// OR-Set — observed-remove set
let set = OrSet::new()
set.add("alice")
set.add("bob")
set.remove("alice")
set.merge(remote_set)  // Handles concurrent add/remove correctly
```

Available CRDTs: `GCounter`, `PNCounter`, `LWWRegister`, `ORSet`.

## Distributed Key-Value Store

```fajar
let kv = DistributedKV::new(replicas: 3, hash_ring_vnodes: 150)

kv.put("user:123", user_data)
let user = kv.get("user:123")  // Routes to correct replica via consistent hashing
```
