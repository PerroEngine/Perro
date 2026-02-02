# Quick Start Guide - Perro LSP Extension

**Note:** perro-lsp is currently non-functional; the extension and server exist for people who want to look at or contribute to them. The goal is autocomplete and type checking using the engine’s API/resource/engine bindings and Pup APIs; LSP development experience is limited, so contributions are welcome. See `perro_lsp/README.md` for more.

## 🚀 Quick Setup (5 minutes)

### Step 1: Build the LSP Server

```bash
cd DIRECTORY\perro
cargo build --release --bin perro-lsp
```

Wait for it to compile. You should see `target\release\perro-lsp.exe` (Windows) or `target/release/perro-lsp` (Linux/Mac).

### Step 2: Install Extension Dependencies

```bash
cd .vscode-extensions\perro-lsp
npm install
```

### Step 3: Compile the Extension

```bash
npm run compile
```

### Step 4: Install the Extension (Permanent Installation)

**This installs the extension so it works everywhere in VSCode automatically:**

```bash
# Make sure you're in the extension directory
cd DIRECTORY\perro\.vscode-extensions\perro-lsp

# Install VSCode Extension Manager (one-time, if you don't have it)
npm install -g vsce

# Package the extension
vsce package

# Install it in VSCode
code --install-extension perro-lsp-0.1.0.vsix
```

**That's it!** Now the extension is installed permanently. You can:
- Open any `.pup` or `.fur` file anywhere in VSCode
- The extension will automatically activate
- No need to run anything special - just use VSCode normally

**Note:** If you update the extension, you'll need to re-package and re-install it.

---

### Alternative: Development Mode (For Testing Changes)

If you're actively developing the extension and want to test changes:

**Option A: From Terminal (Easiest)**

```bash
cd DIRECTORY\perro\.vscode-extensions\perro-lsp
# Open the main perro directory (where target/ folder is) in the new window
code --extensionDevelopmentPath=. --new-window DIRECTORY\perro
```

**Option B: From VSCode UI**

1. **Open the extension folder** in VSCode: `DIRECTORY\perro\.vscode-extensions\perro-lsp`
2. **Press F5** - This opens a new "Extension Development Host" window
3. **IMPORTANT: In the NEW window**, you need to:
   - File → Open Folder
   - Open the **main perro directory**: `DIRECTORY\perro` (NOT a subdirectory like `playground/MessAround`)
   - This is because the extension looks for the LSP server at `target/release/perro-lsp.exe` relative to the workspace root
4. **Then** open a `.pup` or `.fur` file (can be in any subdirectory like `playground/MessAround/res/camera.pup`)

**Why?** The extension needs the workspace root to be the main `perro` directory so it can find `target/release/perro-lsp.exe`. Once that's set up, you can open `.pup` files from any subdirectory.

## ✅ How to Know It's Working

When the extension is working, you should see:

1. **No errors in the Output panel**:
   - View → Output
   - Select "Perro Language Server" from the dropdown
   - Should see "Perro LSP server initialized"

2. **Language features work**:
   - Open a `.pup` file
   - Type some code
   - Press `Ctrl+Space` - you should see code completion suggestions
   - Hover over code - you should see type information
   - Make a syntax error - it should be highlighted with a red squiggle

3. **Problems panel shows diagnostics**:
   - View → Problems (or `Ctrl+Shift+M`)
   - Should show any errors in your `.pup` or `.fur` files

## 🎯 What the Extension Does

The extension provides:

- ✅ **Real-time syntax validation** - Errors appear as you type
- ✅ **Code completion** - Press `Ctrl+Space` for suggestions
- ✅ **Hover information** - Hover to see types and documentation
- ✅ **Error diagnostics** - All errors shown in Problems panel

## 🔧 Troubleshooting

### "Failed to start Perro Language Server"

**Solution**: Make sure the server is built:
```bash
cd DIRECTORY\perro
cargo build --release --bin perro-lsp
```

Then set the server path in VSCode settings:
1. Press `Ctrl+,` to open Settings
2. Search for "perro-lsp"
3. Set "Perro LSP: Server Path" to: `DIRECTORY\perro\target\release\perro-lsp.exe`

### Extension Not Activating

**Check**:
- Do you have a `.pup` or `.fur` file open?
- Check the Output panel (View → Output → "Perro Language Server")
- Look for error messages

### No Code Completion

**Try**:
- Press `Ctrl+Space` manually
- Make sure the file is saved
- Check that the LSP server started (Output panel)

## 📝 Development Workflow

When you make changes:

1. **Change TypeScript code** (`src/extension.ts`):
   - Just press F5 again

2. **Change Rust LSP server** (`perro_lsp/src/`):
   - Rebuild: `cargo build --release --bin perro-lsp`
   - Stop the extension (Shift+F5)
   - Start it again (F5)

## 🎓 Next Steps

- Read `SETUP.md` for detailed information
- Check `perro_lsp/README.md` for LSP server details
- Add more features in `perro_lsp/src/` (diagnostics, completion, etc.)
