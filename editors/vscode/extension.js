const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

/**
 * Debug Adapter Factory — spawns `fj debug --dap` as a subprocess.
 *
 * VS Code calls this when a debug session of type "fajar" is started.
 * The factory creates a DebugAdapterExecutable that communicates via
 * stdin/stdout using the Debug Adapter Protocol (DAP).
 */
class FajarDebugAdapterFactory {
  createDebugAdapterDescriptor(session) {
    const config = vscode.workspace.getConfiguration("fajarLang");
    const fjPath = session.configuration.fjPath || config.get("debug.fjPath", "fj");

    return new vscode.DebugAdapterExecutable(fjPath, ["debug", "--dap"]);
  }
}

/**
 * Formats a Fajar Lang Value for display in the debug variables panel.
 *
 * @param {string} type - The Fajar Lang type name.
 * @param {string} value - The raw value string.
 * @returns {string} Formatted display string.
 */
function formatValueDisplay(type, value) {
  switch (type) {
    case "str":
      return `"${value}"`;
    case "bool":
      return value === "1" || value === "true" ? "true" : "false";
    case "null":
      return "null";
    default:
      return value;
  }
}

function activate(context) {
  // ── LSP Client ──────────────────────────────────────────────
  const config = vscode.workspace.getConfiguration("fajarLang");
  const serverPath = config.get("server.path", "fj");

  const serverOptions = {
    command: serverPath,
    args: ["lsp"],
    transport: TransportKind.stdio,
  };

  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "fajar" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.fj"),
    },
  };

  client = new LanguageClient(
    "fajarLang",
    "Fajar Lang LSP",
    serverOptions,
    clientOptions
  );

  client.start();

  // ── Debug Adapter ───────────────────────────────────────────
  const factory = new FajarDebugAdapterFactory();
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory("fajar", factory)
  );
}

function deactivate() {
  if (client) {
    return client.stop();
  }
}

module.exports = { activate, deactivate, formatValueDisplay };
