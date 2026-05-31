# Leader Election

Cronicle fires scheduled jobs — the nightly invoice run, the 9am reminder email. For high availability it runs several identical scheduler nodes, so if one dies the others carry on. But there's a catch that turns redundancy into a hazard: if all of them are running, all of them fire the 9am email. Every customer gets it three times.

You wanted spare nodes for safety. Now you need exactly one of them to actually do the work.

## The problem

Every `SchedulerNode` is interchangeable. Each wakes on the same `#[schedule = "* * * * *"]` tick, each can call `fireDueJobs`, each is fully capable of running the whole schedule. Capability is not the problem — *coordination* is. The jobs must run, and they must run **once**, but no node can see the others. Left alone, three capable nodes do the same work three times. The pattern's job is to let identical peers agree, without a human in the loop, on a single one of them to act.

## The pattern

The agreement is brokered by a `Coordinator` — a system whose entire purpose is to grant one expiring **lease** and be the single source of truth for who holds it.

Follow `SchedulerNode.tick`. When the cron minute fires, the node does not immediately do work. It first calls `Coordinator.acquire(self.id())`, passing its own `NodeId`. The coordinator answers `Result<void, NotLeader>`: `Ok` to the one node that holds or wins the lease, `Err(NotLeader)` to everyone else. Only on `lease.isOk` does the node call `self.fireDueJobs()`. The losers fall through and stand by, idle but ready — exactly the `OneLeaderFires` feature: "exactly one node holds the lease and fires the due jobs… the others stand by."

The lease is the linchpin, and it *expires*. The leader must keep renewing it. If the leader crashes or hangs and stops renewing, the lease lapses, and at the next `acquire` a standby wins it instead — "if the leader's lease lapses, a standby acquires it and takes over." Failover is automatic and needs no human: leadership is a lease you must keep paying for, and the moment you stop, someone else can claim it.

## When to use it

Reach for leader election when several instances run for availability but a task must execute on exactly one of them: scheduled jobs, a single writer to an external system, ownership of a queue partition, a cluster's coordination role. The shape is always "many warm standbys, one active."

## When to avoid it

If the work is naturally idempotent or partitionable, you may not need a single leader at all — let every node run its slice, or let duplicates be harmless. Avoid election when the `Coordinator` would become a heavier dependency than the availability it buys, or when a brief gap with no leader is unacceptable and a different consistency model fits better.

## Trade-offs

You gain a guaranteed single actor and hands-free failover. You pay with a new dependency every node must reach (`Coordinator`), a failover gap while the lapsed lease is noticed and re-won, and a fistful of hard distributed-systems problems — split-brain, clock skew, fencing — that the next doc takes head-on.
