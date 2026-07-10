# HTTP

## Page Map

| Header    | Link                    |
| --------- | ----------------------- |
| Purpose   | [Purpose](#purpose)     |
| Reference | [Reference](#reference) |

## Purpose

Use `HTTP` when this feature, type group, file format, or workflow appears in game code or assets.

## Reference

# HTTP

Types:

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

## Queue Backpressure

Use bounded queue cfg 4 busy HTTP use:

```rust
let queue = HttpQueueConfig::default()
    .worker_count(4)
    .request_capacity(64)
    .event_capacity(64);
let mut client = HttpClient::with_config_and_queue(HttpConfig::default(), queue);

match client.try_request(HttpRequest::get("https://example.com")) {
    Ok(id) => { /* poll one terminal evt 4 id */ }
    Err(err) if err.kind == HttpSubmitErrorKind::QueueFull => { /* retry aft poll */ }
    Err(err) => { /* req reject b4 queue */ }
}
```

`request` + `get` + verb helpers kp old API.

queue full thru old API -> one local `HTTP_Failed` evt w/ `Send` kind + queue msg.

`try_request` -> direct err + no evt 4 reject.

accepted id -> exact one terminal evt.

## JSON / Variant

```rust
let request = HttpRequest::post_variant(
    "https://example.com/api",
    Variant::from(true),
);

let id = client.request(request);

for event in client.poll_all(8) {
    emit_http_event!(ctx, event);
}
```

## Events

HTTP responses can map to signal params like networking events.

Use `emit_http_event!(ctx, event)` for script signal bridge.
