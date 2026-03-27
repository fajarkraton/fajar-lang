-- =====================================================
-- Fajar Lang Neovim Configuration
-- LSP, file type detection, syntax, keymaps
-- Add to your init.lua or require('fajar') from init.lua
-- =====================================================

local M = {}

-- ── LSP Configuration ──

local function setup_lsp()
  local ok, lspconfig = pcall(require, 'lspconfig')
  if not ok then
    vim.notify('lspconfig not found — install neovim/nvim-lspconfig for Fajar Lang LSP', vim.log.levels.WARN)
    return
  end

  local configs = require('lspconfig.configs')

  if not configs.fajar then
    configs.fajar = {
      default_config = {
        cmd = { 'fj', 'lsp' },
        filetypes = { 'fajar' },
        root_dir = lspconfig.util.root_pattern('fj.toml', '.git'),
        single_file_support = true,
        settings = {},
        init_options = {},
      },
    }
  end

  lspconfig.fajar.setup({
    on_attach = function(client, bufnr)
      -- LSP keymaps (buffer-local)
      local opts = { noremap = true, silent = true, buffer = bufnr }
      vim.keymap.set('n', 'gd', vim.lsp.buf.definition, opts)
      vim.keymap.set('n', 'gD', vim.lsp.buf.declaration, opts)
      vim.keymap.set('n', 'gr', vim.lsp.buf.references, opts)
      vim.keymap.set('n', 'gi', vim.lsp.buf.implementation, opts)
      vim.keymap.set('n', 'K', vim.lsp.buf.hover, opts)
      vim.keymap.set('n', '<leader>rn', vim.lsp.buf.rename, opts)
      vim.keymap.set('n', '<leader>ca', vim.lsp.buf.code_action, opts)
      vim.keymap.set('n', '[d', vim.diagnostic.goto_prev, opts)
      vim.keymap.set('n', ']d', vim.diagnostic.goto_next, opts)
      vim.keymap.set('n', '<leader>e', vim.diagnostic.open_float, opts)
    end,
    capabilities = (function()
      local ok_cmp, cmp_lsp = pcall(require, 'cmp_nvim_lsp')
      if ok_cmp then
        return cmp_lsp.default_capabilities()
      end
      return vim.lsp.protocol.make_client_capabilities()
    end)(),
  })
end

-- ── File Type Detection ──

local function setup_filetype()
  vim.filetype.add({
    extension = {
      fj = 'fajar',
    },
    filename = {
      ['fj.toml'] = 'toml',
    },
  })
end

-- ── Basic Syntax (fallback if no Tree-sitter grammar) ──

local function setup_syntax()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'fajar',
    callback = function()
      vim.bo.commentstring = '// %s'
      vim.bo.tabstop = 4
      vim.bo.shiftwidth = 4
      vim.bo.expandtab = true
      vim.bo.smartindent = true
      vim.bo.suffixesadd = '.fj'
    end,
  })
end

-- ── Keymaps ──

local function setup_keymaps()
  vim.api.nvim_create_autocmd('FileType', {
    pattern = 'fajar',
    callback = function(ev)
      local opts = { noremap = true, silent = true, buffer = ev.buf }

      -- Run current file
      vim.keymap.set('n', '<leader>fr', function()
        vim.cmd('split | terminal fj run ' .. vim.fn.expand('%'))
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Run file' }))

      -- Check current file
      vim.keymap.set('n', '<leader>fc', function()
        vim.cmd('split | terminal fj check ' .. vim.fn.expand('%'))
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Check file' }))

      -- Format current file
      vim.keymap.set('n', '<leader>ff', function()
        vim.cmd('!fj fmt ' .. vim.fn.expand('%'))
        vim.cmd('edit!')
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Format file' }))

      -- Test
      vim.keymap.set('n', '<leader>ft', function()
        vim.cmd('split | terminal fj test')
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Run tests' }))

      -- Build
      vim.keymap.set('n', '<leader>fb', function()
        vim.cmd('split | terminal fj build')
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Build project' }))

      -- Dump tokens
      vim.keymap.set('n', '<leader>fT', function()
        vim.cmd('split | terminal fj dump-tokens ' .. vim.fn.expand('%'))
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Dump tokens' }))

      -- Dump AST
      vim.keymap.set('n', '<leader>fA', function()
        vim.cmd('split | terminal fj dump-ast ' .. vim.fn.expand('%'))
      end, vim.tbl_extend('force', opts, { desc = 'Fajar: Dump AST' }))
    end,
  })
end

-- ── Setup (call from init.lua) ──

function M.setup(opts)
  opts = opts or {}

  setup_filetype()
  setup_syntax()

  if opts.lsp ~= false then
    setup_lsp()
  end

  if opts.keymaps ~= false then
    setup_keymaps()
  end
end

return M
