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
diagnostics on close, and serves whole-document formatting,
nested document symbols, folding ranges, Markdown declaration hover, and
same-file definition locations for the same accepted document version.
Lifecycle events are emitted as one JSON object per line on stderr, leaving
stdout exclusively for LSP framing.

Each open file is currently analyzed independently. Project-root discovery,
resolved local modules and locked packages, partial edit synchronization,
asynchronous request cancellation, completion, and signature help will follow
on the same editor-service boundary.
