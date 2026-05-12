# Script Contexts

Perro script callbacks receive one script context:

- `ctx: &mut ScriptContext<'_, API>`
- runtime window: `ctx.run`
- resource window: `ctx.res`
- input window: `ctx.ipt`
- node id: `ctx.id`

`API` is a script API marker type that implements `ScriptAPI` and binds:
- `API::RT` => runtime API
- `API::RS` => resource API
- `API::IP` => input API

Details:
- [Runtime API](runtime_api.md)
- [Resource API](resource_api.md)
- [Input API](input_api.md)
- [Query System](../query_system.md)

Reference layout:
- `runtime_api.md` contains all runtime scripting macros, signatures, accepted types, and return types.
- `runtime_api.md` contains quick query references and links.
- `query_system.md` contains deeper query concepts, patterns, and performance notes.
- `runtime_api.md` is the RuntimeWindow overview and links to module-specific pages in `runtime_modules/`.
- `resource_api.md` is the ResourceWindow overview and links to module-specific pages in `resource_modules/`.
- `input_api.md` is the InputWindow overview and links to module-specific pages in `input_modules/`.


