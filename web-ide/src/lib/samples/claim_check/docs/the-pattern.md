# The Pattern

A creator uploads a four-gigabyte raw video. The transcoder needs that file — but the message queue between them was never built to carry gigabytes. Claim Check is the move that lets the heavy payload travel one way and the lightweight message travel another, then reunites them.

## The problem

Reelbox is a video-sharing site. Ingest must hand each upload to a `Transcoder` that runs later, asynchronously. The instinct is to put the upload on the queue and let the transcoder pick it up. But message queues are tuned for many small messages, not multi-gigabyte blobs. Stuff a raw video onto the queue and you blow past message-size limits, balloon broker memory, slow every other message behind it, and pay to move the same bytes twice. The queue is the wrong pipe for the payload.

## The pattern

Reelbox sends a *claim*, not the cargo. The `Ingest` container's `receive` takes the raw `bytes` and first calls `BlobStore.put(bytes)`, which returns a `Claim` — a small record holding just a storage `key`. Then it calls `Queue.publish(claim)`. Only that tiny reference rides the queue; the gigabytes go straight to blob storage and never touch the message pipeline.

The two halves stay separate by design. `BlobStore` is its own system — *outside the message pipeline* — exposing `put` and `fetch`. The `Queue` carries only small claims, *never the video bytes*. When a claim arrives, `Transcoder.on` (triggered by `#[onevent(Claim)]`) redeems it: `BlobStore.fetch(claim)` pulls the file back, and `transcode` turns it into stream-ready renditions. The claim is the ticket; the blob store is the coat-check counter.

The `ReferenceNotPayload` feature states the contract: given a multi-gigabyte upload, when Reelbox ingests it, the file is stored in blob storage and only a small claim is queued, and the transcoder redeems the claim to fetch the file — *but the queue never carries the video bytes*. The queue stays fast and cheap regardless of how large any single upload is.

## When to use it

Use it whenever messages would otherwise carry large or variable-size payloads — video, images, documents, big data blobs — through a queue or event bus that has size limits or costs scaled by throughput. It is the standard way to keep an asynchronous pipeline light while still moving heavy data.

## When to avoid it

Skip it when payloads are reliably small; the indirection of store-then-fetch is pure overhead then. Avoid it when consumers need the data inline with zero extra latency, or when introducing a shared blob store you must secure and operate is not worth dodging a modest payload.

## Trade-offs

You trade a single self-contained message for two coordinated steps and a second store. Every consumer now does an extra `fetch`, adding a round trip, and the blob store becomes infrastructure you must provision, secure, and clean up. In return the queue stays small, fast, and cheap no matter how large uploads grow, and producers and consumers move bytes only when they actually need them.
