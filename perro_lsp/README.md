# Perro Language Server

Language Server Protocol (LSP) implementation for `.pup` and `.fur` files in the Perro engine.

## Features

- **Syntax Validation**: Real-time parsing and error detection using the same parser logic as the compiler
- **Type Checking**: Validates types using the same codegen validation logic
- **Code Completion**: Provides autocomplete for:
  - Script variables and functions
  - Node types
  - API modules (Time, JSON, Console, etc.)
  - FUR UI elements
- **Hover Information**: Shows type information and documentation on hover
- **Diagnostics**: Reports errors that would occur during transpilation, including:
  - Parse errors
  - Lifecycle method call errors
  - Type mismatches
  - Invalid API usage

## Architecture

The LSP server reuses the existing parser and validation logic from `perro_core`:

- **Parser Integration**: Uses `PupParser` and `FurParser` directly
- **Validation**: Leverages the same validation checks used during codegen
- **Type System**: Uses the same type inference and checking as the transpiler

This ensures that errors shown in the editor match exactly what would occur during compilation.

## Building

```bash
cargo build --release --bin perro-lsp
```

The executable will be at `target/release/perro-lsp`.

## VSCode Extension

The VSCode extension is located in `.vscode-extensions/perro-lsp/`.

To use it:

1. Build the LSP server (see above)
2. Install the extension:
   ```bash
   cd .vscode-extensions/perro-lsp
   npm install
   npm run compile
   ```
3. In VSCode, open the extension folder and press F5 to run it in a new window

## Configuration

The extension can be configured via VSCode settings:

- `perro-lsp.serverPath`: Path to the `perro-lsp` executable (default: `perro-lsp`)

## Extending

### Adding New Diagnostics

To add new validation checks, modify `src/diagnostics.rs`:

1. Add validation logic in `validate_script`, `validate_statement`, or `validate_expression`
2. Create `Diagnostic` objects with appropriate ranges and messages
3. The diagnostics will automatically be published to the editor

### Adding Completion Items

To add new completion suggestions, modify `src/completion.rs`:

1. Add items to `get_pup_completions` or `get_fur_completions`
2. Use appropriate `CompletionItemKind` values
3. Provide helpful `detail` and `documentation` fields

### Improving Error Positions

The parser already tracks source positions via `SourceSpan`. To improve error reporting:

1. Ensure the parser sets `span` fields on `TypedExpr` and other AST nodes
2. Extract positions from spans in diagnostics
3. Convert `SourceSpan` (1-indexed) to LSP `Position` (0-indexed)

## Integration with Codegen

The LSP server uses the same validation logic as codegen by:

1. Parsing with the same parsers (`PupParser`, `FurParser`)
2. Running similar validation checks (lifecycle methods, type checking, etc.)
3. Using the same type system and API bindings

This ensures compile-time consistency - if the LSP shows an error, the transpiler will also fail.

## Future Improvements

- [ ] Better error position tracking (structured errors from parser)
- [ ] Go-to-definition support
- [ ] Find references
- [ ] Symbol renaming
- [ ] Code actions (quick fixes)
- [ ] Workspace-wide type checking
- [ ] Cross-file references
