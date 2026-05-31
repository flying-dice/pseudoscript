# Edge Cases & Resilience

`Coordinator.acquire` returning `Ok` to one node and `Err(NotLeader)` to the rest looks airtight. It is airtight only as long as time, networks, and the coordinator itself behave — and in a distributed system, none of them fully do.

## Edge cases & failure modes

**Split-brain — two leaders.** The nightmare the pattern exists to prevent, sneaking back in. A network partition cuts the leader off from `Coordinator`. The coordinator, hearing nothing, lapses the lease and grants it to a standby. Now there are two nodes that each believe `lease.isOk` — the old leader (still firing on its stale lease) and the new one. Both run `fireDueJobs`. Duplicate emails, the exact failure redundancy was supposed to avoid.

**Fencing tokens.** The defence against the stale leader. Each lease grant carries a monotonically increasing token; every side effect `fireDueJobs` performs is stamped with it, and the downstream resource rejects any token older than the highest it has seen. The partitioned old leader still *thinks* it leads, but its writes are fenced off — it cannot actually double-run. Election alone is not enough; fencing is what makes single-leader safe.

**Clock skew on lease expiry.** "The lease expires in 30 seconds" means whose 30 seconds? If the leader's clock runs slow it believes it still holds a lease the coordinator already lapsed. Lease timing should be measured by the coordinator's clock, not the nodes', and renew well before expiry to leave slack for skew.

**The coordinator as a dependency.** `Coordinator` is now in the path of every `tick`. If it is down, no node can `acquire`, so no node leads, so no jobs fire. The thing guaranteeing availability has become a single point of failure — which is why real coordinators (a consensus group, not one box) are themselves replicated.

**The failover gap.** Between the leader dying and the lease lapsing, *no one* leads. Jobs due in that window may not fire at all. Shorten the lease and the gap shrinks — but short leases mean more renewal traffic and more sensitivity to clock skew. There is no free failover.

**Work that must not double-run.** Even with one leader, a job fired once can be *delivered* twice downstream. The truly-once guarantee lives at the consumer, not the scheduler.

## Resilience

Tune lease duration against the gap-vs-overhead trade-off, renew on a fraction of that duration, and stamp every effect with a fencing token. Run `Coordinator` as a replicated consensus service so it is not the weak link. Make `fireDueJobs` and its downstream effects idempotent so a brief two-leader window or a redelivery does no harm.

## Pairs well with

Leader election pairs naturally with **fencing tokens** (above) and with an **idempotent receiver** downstream, so a double-fired job is absorbed rather than duplicated. It often sits on top of a **consensus** protocol (Raft, Paxos) that implements `Coordinator`, and complements a **heartbeat / lease-renewal** mechanism that detects the dead leader in the first place.
