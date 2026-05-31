# Edge Cases & Resilience

`ReferenceNotPayload` keeps the queue light by splitting the data from the message. The split is exactly where things go wrong: now two stores must stay in sync, and the claim can outlive what it points to.

## Edge cases & failure modes

**Orphaned blobs.** `Ingest.receive` calls `BlobStore.put` *then* `Queue.publish`. If it crashes between them, the bytes sit in blob storage but no claim was ever queued — an orphan no consumer will ever fetch, silently costing storage forever. The reverse is just as bad: publish a claim, then fail to confirm the put, and the `Transcoder` redeems a claim whose blob is missing.

**The claim outliving the payload.** A claim is just a `key`. Nothing in the message guarantees the blob still exists when the `Transcoder` finally calls `BlobStore.fetch(claim)`. If a lifecycle policy or cleanup job deletes the blob while the claim is still queued — or being retried after a failure — the consumer holds a valid-looking ticket for a coat that has already been thrown out. `fetch` returns nothing, and the job fails in a way the queue alone cannot explain.

**TTL and cleanup.** Blob TTLs must outlast the worst-case queue dwell time *plus* all retries, or you recreate the previous failure. And successfully transcoded blobs need deleting, or storage grows without bound — but delete too eagerly and an in-flight retry loses its source.

**Security of the reference.** The `Claim` carries a storage `key`, and a key is an authority. If it leaks — logged, forwarded, guessable — anyone holding it may fetch a private upload directly from `BlobStore`, bypassing Reelbox entirely. The claim must be unguessable, scoped to the one blob, and ideally short-lived (a signed, expiring URL rather than a raw permanent key).

## Resilience

Order the writes so blob-first means a failed publish leaves only a cheap, reclaimable orphan — then run a sweeper that deletes blobs with no live claim. Make `Transcoder.on` tolerate a missing blob explicitly rather than crashing, and dead-letter claims whose payload has vanished. Keep blob TTLs strictly longer than queue retention plus retry budget. Treat the `key` as a credential: unguessable, least-privilege, expiring.

## Pairs well with

**Competing Consumers** is the usual reader: a pool of transcoders drains the claim queue in parallel, each redeeming its own claim — the small messages keep the queue fast to dequeue.

**Publish/Subscribe** fans a single claim out to several consumers (transcode, thumbnail, virus-scan), each fetching the same blob independently from `BlobStore`.

**Transactional Outbox** closes the orphan gap on the producer side: commit the blob reference to an outbox in the same transaction as the upload record, so the claim is published reliably rather than best-effort.
