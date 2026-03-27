# IDE Setup Guide

Fajar Lang has editor support through an LSP server and syntax highlighting
packages for popular editors.

## VS Code (Recommended)

### Install the Extension

1. Open VS Code
2. Go to Extensions (`Ctrl+Shift+X`)
3. Search for "Fajar Lang"
4. Install the `fajar-lang` extension

Or install from the command line:

```bash
code --install-extension fajar-lang.fajar-lang
```

### Features

- Syntax highlighting for `.fj` files
- Code snippets (`fn`, `struct`, `match`, `test`, etc.)
- LSP integration (diagnostics, go-to-definition, hover, rename)
- Format on save (uses `fj fmt`)

### Configuration

Add to `settings.json`:

```json
{
    "fajar-lang.server.path": "/usr/local/bin/fj",
    "fajar-lang.format.onSave": true,
    "fajar-lang.check.onSave": true,
    "editor.semanticHighlighting.enabled": true
}
```

### Starting the LSP Server Manually

```bash
fj lsp
```

The extension starts this automatically, but you can run it manually
for debugging.

## Neovim

### Using nvim-lspconfig

Add to your Neovim config (`init.lua`):

```lua
local lspconfig = require('lspconfig')

lspconfig.fajar_lang.setup {
    cmd = { "fj", "lsp" },
    filetypes = { "fajar" },
    root_dir = lspconfig.util.root_pattern("fj.toml"),
}

vim.filetype.add({
    extension = { fj = "fajar" },
})
```

### Syntax Highlighting with Tree-sitter

```lua
-- In your tree-sitter config
require('nvim-treesitter.configs').setup {
    ensure_installed = { "fajar" },
    highlight = { enable = true },
}
```

## Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "fajar"
scope = "source.fajar"
file-types = ["fj"]
roots = ["fj.toml"]
language-server = { command = "fj", args = ["lsp"] }
indent = { tab-width = 4, unit = "    " }
comment-token = "//"
```

## JetBrains IDEs (IntelliJ, CLion, RustRover)

1. Install the "Fajar Lang" plugin from the JetBrains Marketplace
2. Configure the LSP server path in Settings > Languages > Fajar Lang
3. Set the path to `fj` binary

### Manual LSP Configuration

In Settings > Languages & Frameworks > Language Server Protocol:

- Server: `/usr/local/bin/fj lsp`
- File patterns: `*.fj`

## Sublime Text

Install the `Fajar` package via Package Control, or manually copy the
syntax file to your Packages directory:

```bash
cp editors/sublime/Fajar.sublime-syntax \
   ~/.config/sublime-text/Packages/User/
```

## Universal LSP Features

All editors with LSP support get:

| Feature | Description |
|---------|-------------|
| Diagnostics | Real-time error and warning display |
| Go to Definition | Jump to function/type definitions |
| Hover | Show type information on hover |
| Rename | Rename symbols across files |
| Completion | Basic code completion |
| Formatting | Format with `fj fmt` |
