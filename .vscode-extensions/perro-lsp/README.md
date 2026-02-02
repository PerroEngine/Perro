# Perro LSP VSCode Extension

VSCode extension that provides Language Server Protocol support for `.pup` and `.fur` files.

## Current status

**perro-lsp is currently non-functional.** The extension and server exist for anyone who wants to look at them or contribute. The goal is to use the engine’s API bindings, resource bindings, engine bindings, and Pup APIs for autocomplete and type checking; LSP development experience is limited, so contributions are welcome. See `perro_lsp/README.md` in the main repo for more.

## Setup

1. **Build the LSP server**:
   ```bash
   cd DIRECTORY\perro
   cargo build --release --bin perro-lsp
   ```

2. **Install extension dependencies**:
   ```bash
   cd .vscode-extensions/perro-lsp
   npm install
   ```

3. **Compile the extension**:
   ```bash
   npm run compile
   ```

4. **Run the extension**:
   - Open the `.vscode-extensions/perro-lsp` folder in VSCode
   - Press F5 to launch a new Extension Development Host window
   - Open a `.pup` or `.fur` file to test

## Configuration

The extension can be configured via VSCode settings:

- `perro-lsp.serverPath`: Path to the `perro-lsp` executable
  - Default: `perro-lsp` (assumes it's in PATH or in `target/release/` or `target/debug/`)
  - Can be an absolute path or relative to workspace root

## Troubleshooting

### Server not starting

If you see "Failed to start Perro Language Server":

1. Make sure the server is built:
   ```bash
   cargo build --release --bin perro-lsp
   ```

2. Check that the executable exists:
   - Windows: `target\release\perro-lsp.exe`
   - Linux/Mac: `target/release/perro-lsp`

3. Set the `perro-lsp.serverPath` setting to the full path if needed

### Extension not activating

- Make sure you have a `.pup` or `.fur` file open
- Check the VSCode Output panel for errors
- Verify the extension is enabled in the Extensions view
