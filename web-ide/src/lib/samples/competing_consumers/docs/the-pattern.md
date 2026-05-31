# The Pattern

A photo-sharing site gets a thousand uploads in a minute, then twelve in the next. Some images are tiny; one is a hundred-megapixel monster that takes a full second to resize. Competing Consumers is how Thumbly absorbs both the spike and the straggler without anyone tuning a thing.

## The problem

Thumbly turns uploads into thumbnails. The tempting design has the upload endpoint resize the image inline, in the request. Two problems appear immediately. First, a burst of uploads means a burst of simultaneous resizes, and the box falls over. Second, that hundred-megapixel image blocks its request for a full second — and if resizing happens inline, it blocks everything queued behind it on that thread. Throughput is hostage to the slowest image and the spikiest minute.

## The pattern

Thumbly separates *accepting* work from *doing* it. The `Intake` container's `submit` does almost nothing: it takes an `Upload` and calls `Queue.enqueue(upload)`, then returns. The `#[http("POST /upload")]` request is fast and steady no matter how heavy the image is, because intake never resizes anything itself.

The work waits in the `Queue`. Its shape is the heart of the pattern: `enqueue` puts a job in, and `dequeue(): Option<Upload>` hands the next job to a free worker — returning `None` when the queue is empty. The `Option` matters. A worker that finds nothing simply goes back to sleep.

The consumers compete for those jobs. `Resizer` is *one of a pool of interchangeable workers* — the model is deliberately a single definition you run many copies of. Each `Resizer.poll` (on a `#[schedule]`) calls `Queue.dequeue`, and if it got a job (`next.isSome`) it resizes it. Because the queue hands each job to exactly one worker, two resizers never duplicate the same image.

The `OneJobOneWorker` feature states the contract: given a backlog and several workers, each upload is resized by exactly one worker, adding workers clears the backlog faster — *but a huge image ties up only its own worker*. That final clause is the win the inline design could never offer. The hundred-megapixel monster occupies one `Resizer` for a second; the other workers keep draining the queue around it.

## When to use it

Use it when work is independent and parallelizable, when load is bursty, and when you want to scale throughput by simply adding workers rather than rewriting code. It is the default shape for background jobs: image processing, email sending, report generation, any "do this later, in bulk."

## When to avoid it

Avoid it when jobs for a given key must run in strict order — competing workers process in parallel and order is lost. Avoid it when each job needs a synchronous result returned to the caller, or when the work is so cheap that a queue hop costs more than the job itself.

## Trade-offs

You trade simplicity and ordering for elasticity and isolation. A direct call is easier to read and trace than enqueue-and-poll. In return you scale horizontally for free, you isolate slow jobs to a single worker, and a crashing worker takes down only the one job it held. The queue becomes a buffer that smooths every spike — at the cost of a component you must now run and watch.
