# HTTP

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |
| Client | [Client](#client) |
| Queue Backpressure | [Queue Backpressure](#queue-backpressure) |
| JSON / Variant | [JSON / Variant](#json-variant) |
| Events | [Events](#events) |

## Purpose

`HttpClient` runs request/response HTTP against web services on a background
worker pool, so a slow or unreachable server never stalls the game loop. You
submit a request, keep playing, and poll for the response as an event a few
frames later. Bodies can be raw bytes, text, or a `Variant` that serializes to
and from JSON, which covers most game backends.

## Use Cases

- Message of the day / news: `client.get(url)` at startup and show the response
  text when it arrives.
- Submit a score or event: `HttpRequest::post_variant(url, value)` to send JSON
  to a leaderboard or analytics endpoint.
- Patch / version check: send a `Get` (or `Head`) and branch on
  `response.status` / `response.ok()`.
- Telemetry under load: use `try_request` with a bounded queue so a burst of
  events applies backpressure instead of unbounded memory growth.
- Authenticated calls: attach headers such as
  `HttpRequest::get(url).header("Authorization", "Bearer ...")`.

## Practical Example

```rust
use std::cell::RefCell;

thread_local! {
    static HTTP: RefCell<HttpClient> = RefCell::new(HttpClient::new());
}

lifecycle!({
    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {
        // Fire a request; the worker pool handles it off the game thread.
        HTTP.with(|http| {
            http.borrow_mut().get("https://example.com/motd");
        });
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Poll finished requests and forward them as signals.
        let events = HTTP.with(|http| http.borrow_mut().poll_all(8));
        for event in events {
            emit_http_event!(ctx.run, event);
        }
    }
});
```

## Reference

HTTP lives in `perro_api::networking`. Core types:

- `HttpClient`
- `HttpConfig`
- `HttpQueueConfig`
- `HttpRequest`
- `HttpResponse`
- `HttpEvent`
- `HttpError`
- `HttpErrorKind`
- `HttpSubmitError`
- `HttpSubmitErrorKind`

## Client

```rust
let mut client = HttpClient::new();
let id = client.request(HttpRequest::get("https://example.com"));
```

`request` submits work and returns an `HttpID`. Verb helpers (`get`,
`post_variant`, `put_variant`, `patch_variant`, `post_bytes`) wrap
`HttpRequest`. Every accepted request yields exactly one terminal event
(`HttpEvent::Completed` or `HttpEvent::Failed`) carrying the same id. Responses
in the 400-599 range arrive as `Failed` with `HttpErrorKind::Status`.

Read a response body with `response.text()`, `response.variant()` (JSON), or
`response.bytes()`; check success with `response.ok()`.

## Queue Backpressure

Use a bounded queue configuration for busy HTTP workloads:

```rust
let queue = HttpQueueConfig::default()
    .worker_count(4)
    .request_capacity(64)
    .event_capacity(64);
let mut client = HttpClient::with_config_and_queue(HttpConfig::default(), queue);

match client.try_request(HttpRequest::get("https://example.com")) {
    Ok(id) => { /* poll for one terminal event for this id */ }
    Err(err) if err.kind == HttpSubmitErrorKind::QueueFull => { /* retry after polling */ }
    Err(err) => { /* request rejected before it reached the queue */ }
}
```

Behavior:

- `request` keeps the simple API: a full queue produces one local
  `HTTP_Failed` event with the `Send` kind and a queue message.
- `try_request` returns the error directly and produces no event on rejection.
- Each accepted id produces exactly one terminal event.

## JSON / Variant

```rust
let request = HttpRequest::post_variant(
    "https://example.com/api",
    Variant::from(true),
);

let id = client.request(request);

for event in client.poll_all(8) {
    emit_http_event!(ctx.run, event);
}
```

A `Variant` body serializes to JSON and sets `Content-Type: application/json`
unless the request already carries that header.

## Events

HTTP responses map to signal params like networking events. Bridge them into
script signals with `emit_http_event!(ctx.run, event)`. `HTTP_Completed` carries
the id, status, url, and body; `HTTP_Failed` carries the id, url, error kind, and
message.
