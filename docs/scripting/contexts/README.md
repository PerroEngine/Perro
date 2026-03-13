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
- `runtime_context.md` is the RuntimeContext overview and links to module-specific pages in `runtime_modules/`.
- `resource_context.md` is the ResourceContext overview and links to module-specific pages in `resource_modules/`.
- `input_context.md` is the InputContext overview and links to module-specific pages in `input_modules/`.
