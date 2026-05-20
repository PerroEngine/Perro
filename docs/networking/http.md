# HTTP

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `HTTP` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

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
