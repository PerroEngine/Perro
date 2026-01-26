import * as path from 'path';
import * as fs from 'fs';
import { workspace, window, ExtensionContext } from 'vscode';
import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext) {
	// Get the path to the LSP server executable
	const config = workspace.getConfiguration('perro-lsp');
	const serverPath = config.get<string>('serverPath', 'perro-lsp');
	
	// Resolve the server executable path
	let serverExecutable: string;
	if (path.isAbsolute(serverPath)) {
		serverExecutable = serverPath;
	} else {
		// Try to find the executable in the workspace root's target directory
		const workspaceRoot = workspace.workspaceFolders?.[0]?.uri.fsPath;
		if (workspaceRoot) {
			// Try release build first
			const releasePath = path.join(workspaceRoot, 'target', 'release', serverPath);
			const debugPath = path.join(workspaceRoot, 'target', 'debug', serverPath);
			
			// Add .exe extension on Windows
			const releaseExe = process.platform === 'win32' ? `${releasePath}.exe` : releasePath;
			const debugExe = process.platform === 'win32' ? `${debugPath}.exe` : debugPath;
			
			if (fs.existsSync(releaseExe)) {
				serverExecutable = releaseExe;
			} else if (fs.existsSync(debugExe)) {
				serverExecutable = debugExe;
			} else {
				// Fallback to just the name (assumes it's in PATH)
				serverExecutable = serverPath;
			}
		} else {
			// No workspace, assume it's in PATH
			serverExecutable = serverPath;
		}
	}

	// Server options
	// Use the same executable for both run and debug modes
	// For debugging, you can attach a debugger to the running process
	const serverOptions: ServerOptions = {
		run: { 
			command: serverExecutable, 
			args: [], 
			transport: TransportKind.stdio 
		},
		debug: {
			// Use the same executable - if you need to debug, build in debug mode first
			// and attach a debugger, or use the debug executable if available
			command: serverExecutable,
			args: [],
			transport: TransportKind.stdio,
		}
	};

	// Options to control the language client
	const clientOptions: LanguageClientOptions = {
		// Register the server for .pup and .fur files
		documentSelector: [
			{ scheme: 'file', language: 'pup' },
			{ scheme: 'file', language: 'fur' }
		],
		synchronize: {
			// Notify the server about file changes to .pup and .fur files contained in the workspace
			fileEvents: workspace.createFileSystemWatcher('**/*.{pup,fur}')
		}
	};

	// Create the language client
	client = new LanguageClient(
		'perroLsp',
		'Perro Language Server',
		serverOptions,
		clientOptions
	);

	// Start the client. This will also launch the server
	try {
		await client.start();
	} catch (error) {
		console.error('Failed to start Perro Language Server:', error);
		// Show error to user
		window.showErrorMessage(
			`Failed to start Perro Language Server. Make sure the server is built: cargo build --release --bin perro-lsp`
		);
	}
}

export async function deactivate(): Promise<void> {
	if (client) {
		await client.stop();
		client = undefined;
	}
}
