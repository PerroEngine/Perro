# HTTP

Types:

- `HttpClient`
- `HttpConfig`
- `HttpRequest`
- `HttpResponse`
- `HttpEvent`
- `HttpError`
- `HttpErrorKind`

## Client

```rust
let mut client = HttpClient::new();
let id = client.request(HttpRequest::get("https://example.com"));
```

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
