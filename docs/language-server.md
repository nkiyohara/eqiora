# Eqiora language server

`eqiora-language-server` is an editor-independent LSP preview backed by
Eqiora's compiler-owned editor analysis service.

Install it from a checkout:

```console
cargo install --locked --path crates/eqiora-language-server
eqiora-language-server --version
```

Configure an LSP client to start `eqiora-language-server` over stdio for `.eqi`
files. For example, Neovim 0.11 can start it from `ftplugin/eqiora.lua`:

```lua
vim.lsp.start({
  name = "eqiora",
  cmd = { "eqiora-language-server" },
  root_dir = vim.fs.root(0, { "eqiora.toml", ".git" }) or vim.fn.getcwd(),
})
```

The preview uses standard UTF-16 LSP positions and full-document synchronization.
Each document has a 16 MiB analysis limit. The server publishes ordered
parser/compiler diagnostics after open and accepted newer changes, clears
diagnostics on close, and serves whole-document formatting, nested document
symbols, folding ranges, Markdown declaration hover, and definition locations.
Lifecycle events are emitted as one JSON object per line on stderr, leaving
stdout exclusively for LSP framing.

Files opened under the same initialization workspace folder are analyzed as one
module graph. When that folder contains `eqiora.toml`, the server loads its exact
local package graph from disk, including unopened sources, without writing a lock
or package store. Open model sources override their disk content until they are
closed, so hover and definition navigation stay current after full-document
changes. Workspace analysis runs on one background worker, coalesces pending
edits, and prevents superseded results from publishing diagnostics. Partial edits
are planned next.
