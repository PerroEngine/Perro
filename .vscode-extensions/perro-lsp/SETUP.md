# Perro LSP Extension Setup Guide

This guide explains how to set up and use the Perro Language Server extension in VSCode.

## How It Works

The extension consists of two parts:

1. **LSP Server** (Rust binary): The actual language server that parses `.pup` and `.fur` files, validates them, and provides language features
2. **VSCode Extension** (TypeScript): The client that communicates with the LSP server and integrates it into VSCode

The extension uses the Language Server Protocol (LSP) to communicate between VSCode and the Rust server via stdio.

## Setup Steps

### 1. Build the LSP Server

First, you need to compile the Rust LSP server:

```bash
cd DIRECTORY\perro
cargo build --release --bin perro-lsp
```

This will create the executable at:
- Windows: `target\release\perro-lsp.exe`
- Linux/Mac: `target/release/perro-lsp`

### 2. Install Extension Dependencies

Navigate to the extension directory and install Node.js dependencies:

```bash
cd .vscode-extensions/perro-lsp
npm install
```

### 3. Compile the Extension

Compile the TypeScript extension code:

```bash
npm run compile
```

This creates the JavaScript files in the `out/` directory.

## Using the Extension

### Option 1: Permanent Installation (Recommended)

**Install it once, use it everywhere!** This is what you want if you just want the extension to work automatically whenever you open `.pup` or `.fur` files.

1. **Package the extension**:
   ```bash
   cd DIRECTORY\perro\.vscode-extensions\perro-lsp
   npm install -g vsce  # Install VSCode Extension Manager (one-time)
   vsce package
   ```
   This creates a `.vsix` file (e.g., `perro-lsp-0.1.0.vsix`).

2. **Install the extension** (choose one method):

   **Method A: From Terminal**
   ```bash
   code --install-extension perro-lsp-0.1.0.vsix
   ```

   **Method B: From VSCode UI**
   - In VSCode, go to Extensions view (Ctrl+Shift+X)
   - Click the "..." menu at the top
   - Select "Install from VSIX..."
   - Choose the `.vsix` file you just created

3. **Reload VSCode** when prompted

**Done!** Now the extension is installed permanently. You can:
- Open any `.pup` or `.fur` file anywhere in VSCode
- The extension will automatically activate
- No special commands needed - just use VSCode normally

**To update:** Re-package and re-install when you make changes to the extension.

---

### Option 2: Development Mode (For Testing Changes)

Use this if you're actively developing the extension and want to test changes without reinstalling.

**From Terminal:**
```bash
cd DIRECTORY\perro\.vscode-extensions\perro-lsp
code --extensionDevelopmentPath=. --new-window DIRECTORY\perro\projects\MessAround
```

**From VSCode UI:**
1. Open the extension folder in VSCode: `DIRECTORY\perro\.vscode-extensions\perro-lsp`
2. Press **F5** (or Run → Start Debugging)
3. A new "Extension Development Host" window opens
4. **IMPORTANT: In the NEW window**, you need to:
   - File → Open Folder
   - Open the **main perro directory**: `DIRECTORY\perro` (NOT a subdirectory)
   - This is because the extension looks for the LSP server at `target/release/perro-lsp.exe` relative to the workspace root
5. **Then** open a `.pup` or `.fur` file (can be in any subdirectory like `projects/MessAround/res/camera.pup`)

**To stop:** Press Shift+F5 in the original VSCode window

**Why the main directory?** The extension needs the workspace root to be `DIRECTORY\perro` so it can find the LSP server executable at `target/release/perro-lsp.exe`. Once that's set up, you can open `.pup` files from any subdirectory.

## Configuration

The extension looks for the LSP server executable in this order:

1. The path specified in `perro-lsp.serverPath` setting (if absolute)
2. `target/release/perro-lsp` (or `perro-lsp.exe` on Windows) in workspace root
3. `target/debug/perro-lsp` (or `perro-lsp.exe` on Windows) in workspace root
4. `perro-lsp` in your system PATH

To configure the server path manually:

1. Open VSCode Settings (Ctrl+,)
2. Search for "perro-lsp"
3. Set `Perro LSP: Server Path` to the full path to your executable

Or add to your `settings.json`:
```json
{
  "perro-lsp.serverPath": "DIRECTORY\\perro\\target\\release\\perro-lsp.exe"
}
```

## Features

Once running, the extension provides:

- **Syntax Validation**: Errors are shown in real-time as you type
- **Code Completion**: Press Ctrl+Space to see suggestions for:
  - Variables and functions in your script
  - Node types
  - API modules (Time, JSON, Console, etc.)
  - FUR UI elements
- **Hover Information**: Hover over code to see type information
- **Diagnostics**: Errors appear in the Problems panel (View → Problems)

## Troubleshooting

### Extension Not Activating

- Make sure you have a `.pup` or `.fur` file open
- Check the Output panel: View → Output → Select "Perro Language Server" from dropdown
- Look for error messages

### Server Not Starting

- Verify the server is built: `cargo build --release --bin perro-lsp`
- Check that the executable exists at the expected path
- Set `perro-lsp.serverPath` to the full path if needed
- Check the Output panel for error messages

### No Language Features

- Make sure the extension is activated (check the Output panel)
- Try restarting the LSP server: Command Palette (Ctrl+Shift+P) → "Developer: Restart Extension Host"
- Check that your file has the `.pup` or `.fur` extension

## Development Workflow

When developing the extension:

1. **Make changes to TypeScript**:
   - Edit `src/extension.ts`
   - Press F5 to test (automatically recompiles)

2. **Make changes to Rust LSP server**:
   - Edit files in `perro_lsp/src/`
   - Rebuild: `cargo build --release --bin perro-lsp`
   - Restart the extension (Shift+F5, then F5 again)

3. **Debug the LSP server**:
   - The extension is configured to use `cargo run` in debug mode
   - Set breakpoints in Rust code
   - Use VSCode's Rust debugger or `rust-gdb`

## File Structure

```
.vscode-extensions/perro-lsp/
├── src/
│   └── extension.ts          # Extension entry point
├── out/                      # Compiled JavaScript (generated)
├── package.json              # Extension manifest
├── tsconfig.json             # TypeScript config
└── README.md                 # This file

perro_lsp/                    # Rust LSP server
├── src/
│   ├── main.rs              # Server entry point
│   ├── server.rs            # LSP server implementation
│   ├── diagnostics.rs        # Error checking
│   ├── completion.rs         # Code completion
│   └── ...
└── Cargo.toml
```

## Next Steps

- Add more validation rules in `perro_lsp/src/diagnostics.rs`
- Add more completion items in `perro_lsp/src/completion.rs`
- Add hover information in `perro_lsp/src/hover.rs`
- Add go-to-definition, find references, etc.
