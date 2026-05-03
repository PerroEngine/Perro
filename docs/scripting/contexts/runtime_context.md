# Runtime Context

Type:

- `ctx: &mut ScriptContext<'_, API>`
- runtime window handle: `ctx.run`

Runtime macros take `ctx.run` as argument 1.

## Runtime Modules

- [Time Module](runtime_modules/time.md)
- [Nodes Module](runtime_modules/nodes.md)
- [Animations Module](runtime_modules/animations.md)
- [Scripts Module](runtime_modules/scripts.md)
- [Signals Module](runtime_modules/signals.md)
- [Physics Module](runtime_modules/physics.md)
- [Helpers](runtime_modules/helpers.md)

Each module page contains:
- Macro reference
- Signature notes
- Examples
- Behavioral guidance (ownership, mutability, IDs, and query/inheritance usage)


