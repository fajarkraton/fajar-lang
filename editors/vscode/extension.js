const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
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
}

function deactivate() {
  if (client) {
    return client.stop();
  }
}

module.exports = { activate, deactivate };
