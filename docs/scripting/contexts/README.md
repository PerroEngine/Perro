# Script Contexts

Perro script callbacks receive three context objects:

- `ctx: &mut RuntimeContext<'_, RT>`
- `res: &ResourceContext<'_, RS>`
- `ipt: &InputContext<'_, IP>`

Details:
- [Runtime Context](runtime_context.md)
- [Resource Context](resource_context.md)
- [Input Context](input_context.md)

Reference layout:
- `runtime_context.md` contains all runtime scripting macros, signatures, accepted types, and return types.
- `runtime_context.md` also contains `query!` forms, predicates, and examples.
- `resource_context.md` and `input_context.md` document their full macro sets, method signatures, and examples.
