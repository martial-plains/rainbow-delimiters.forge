# rainbow-delimiters

Colour `()`, `[]` and `{}` by nesting depth, so matching pairs are easy to
see. A port of [Fanael/rainbow-delimiters](https://github.com/Fanael/rainbow-delimiters)
(GPL-3.0-or-later, by Jeremy Rayman and Fanael Linithien) to a Forge plugin.

| Command                          | What it does                                             |
|----------------------------------|----------------------------------------------------------|
| `M-x rainbow-delimiters-mode`    | Toggle in this buffer (prefix: positive on, ≤ 0 off)     |
| `M-x global-rainbow-delimiters-mode` | Every file buffer except plain text, now and later   |

Also under **View → Rainbow Delimiters**.

To switch it on from `~/.config/forge/init.rhai`, use a mode hook, as
Emacs users do with `(add-hook 'prog-mode-hook #'rainbow-delimiters-mode)`:

```rhai
forge.mode.hook("prog-mode", "rainbow-delimiters-mode");
```

That turns it on in every buffer whose mode derives from `prog-mode`: all
the programming modes, now and for files opened later. Name a narrower
mode for fewer (`"rust-mode"`), or turn on the global mode instead:

```rhai
forge.command.run("global-rainbow-delimiters-mode", 1);   // 1: on, not toggle
```

Both work although plugins load after init.rhai. The hook runs, and the
command waits, until the plugin has loaded.

## Install with use_package

Add this to `~/.config/forge/init.rhai`:

```rhai
use_package("rainbow-delimiters", #{
  vc: "https://github.com/martial-plains/forge-plugin-rainbow-delimiters",
  pin: "v0.1.0",
  commands: ["rainbow-delimiters-mode", "global-rainbow-delimiters-mode"],
  hook: #{ "prog-mode": "rainbow-delimiters-mode" },
});
```

Forge asks before running the package build command the first time.

## Build

    cargo build --release --target wasm32-wasip2
    cp target/wasm32-wasip2/release/rainbow_delimiters.wasm ~/.config/forge/plugins/rainbow-delimiters.wasm

or `scripts/install-plugins.sh rainbow-delimiters`. `cargo test` runs the
bracket-scanning tests natively. It needs no permissions.

## Faces

`rainbow-depth-1` … `rainbow-depth-9`, `rainbow-unmatched` (a closer with
no opener) and `rainbow-mismatched` (`(]`). They need the face support in
`rainbow-faces.patch` (see below); restyle them with `forge.theme`, as
Emacs users do with `rainbow-delimiters-depth-N-face`.

On a Forge without them, the plugin notices the unknown face and cycles
through the syntax faces (`keyword`, `string`, …) instead, with `error` and
`warning` for the unmatched/mismatched ones. It works, but the colours are
the same as code tokens.

## What differs from the Emacs package

- Strings, comments and escapes are skipped using Forge's tree-sitter
  highlighting, where Emacs uses the syntax table. A buffer whose mode has
  no grammar (Lisp, say) has every bracket coloured.
- Recolouring happens when typing pauses (Forge's `buffer-changed` is sent
  after 300 ms idle), over the whole buffer rather than the visible part.
  Buffers over 4 MB are left alone.
- No `rainbow-delimiters-pick-face-function` or `…-max-face-count`
  settings: plugins have no settings API yet. The two counts are constants
  at the top of `src/lib.rs`.
- No two-character-opener handling (`#(`, `{|`): Forge has no syntax tables.


## Building

This plugin is built against the Forge plugin SDK (`forge-plugin-sdk`),
taken from the Forge repository named in `Cargo.toml` (change the `git =`
URL if Forge lives elsewhere).

    rustup target add wasm32-wasip2
    cargo build --release --target wasm32-wasip2

Copy the resulting `.wasm` to `~/.config/forge/plugins/`.

## License

GPL-3.0-or-later. See the license file in this directory.
