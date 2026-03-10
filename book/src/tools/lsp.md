# LSP Server

Fajar Lang includes a Language Server Protocol implementation for editor integration.

## Starting the Server

```bash
fj lsp
```

## Features

- **Diagnostics**: Real-time error highlighting (lex, parse, semantic errors)
- **Go-to-definition**: Jump to function and type definitions
- **Hover**: Type information on hover
- **Auto-completion**: Suggest function names, keywords, and types

## VS Code Setup

1. Install the Fajar Lang extension
2. The extension automatically starts `fj lsp` in the background
3. Open any `.fj` file to see diagnostics

## Editor Support

The LSP server works with any editor that supports the Language Server Protocol:

- VS Code
- Neovim (via nvim-lspconfig)
- Emacs (via lsp-mode)
- Helix
- Sublime Text (via LSP package)
