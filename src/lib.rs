// SPDX-License-Identifier: GPL-3.0-or-later
//! rainbow-delimiters: colour brackets by how deeply they're nested.
//!
//! A port of Fanael Linithien's Emacs package of the same name
//! (GPL-3.0-or-later). Each level of `()`, `[]` and `{}` gets its own
//! colour, so matching pairs are easy to spot; a closing bracket with no
//! opener, or the wrong kind of one, is flagged.
//!
//! - `M-x rainbow-delimiters-mode` toggles it in this buffer. With a prefix
//!   argument, a positive one turns it on and zero or a negative one off,
//!   as in Emacs.
//! - `M-x global-rainbow-delimiters-mode` turns it on in every file buffer
//!   now and in ones opened later, except plain text (`text-mode`,
//!   `org-mode`, `fundamental-mode`) where brackets are mostly prose.
//!
//! Brackets inside strings and comments are left alone when the buffer's
//! major mode has a grammar (the same tree-sitter highlighting Forge
//! draws). A buffer with no grammar is treated as all code.
//!
//! The colours are the faces `rainbow-depth-1` to `rainbow-depth-9`,
//! `rainbow-unmatched` and `rainbow-mismatched`, which a theme can restyle
//! (`forge.theme`). A Forge that doesn't have those faces yet falls back to
//! cycling through the syntax faces, so the plugin still works there.
//!
//!   cargo build --release --target wasm32-wasip2
//!   cp target/wasm32-wasip2/release/forge_rainbow_delimiters.wasm ~/.config/forge/plugins/rainbow-delimiters.wasm

pub mod delimiters;

use delimiters::{Face, Skip};
use forge_plugin_sdk::{
    Buffer, Decoration, Event, EventKind, Position, Span, command, decorations, extend_submenu,
    forge_plugin, menu_item, message, modes, on,
};
use std::cell::Cell;

pub const MODE: &str = "rainbow-delimiters-mode";

/// Faces to cycle through before wrapping round
/// (`rainbow-delimiters-max-face-count`).
pub const MAX_FACE_COUNT: usize = 9;
/// Faces kept for the outermost levels only
/// (`rainbow-delimiters-outermost-only-face-count`).
pub const OUTERMOST_ONLY_FACE_COUNT: usize = 0;
/// Buffers bigger than this aren't coloured (the whole text is scanned
/// each time it changes).
const MAX_BYTES: usize = 4_000_000;

/// What stands in for the depth faces on a Forge that doesn't know them.
const FALLBACK_DEPTH: [&str; 9] = [
    "keyword",
    "string",
    "function",
    "type",
    "constant",
    "builtin",
    "property",
    "attribute",
    "escape",
];

thread_local! {
    static GLOBAL: Cell<bool> = const { Cell::new(false) };
    /// This Forge rejected a rainbow face: use the syntax faces instead.
    static FALLBACK: Cell<bool> = const { Cell::new(false) };
}

/// The name of the face, for `decorations.set`.
pub fn face_name(face: Face, fallback: bool) -> String {
    match (face, fallback) {
        (Face::Depth(n), false) => format!("rainbow-depth-{n}"),
        (Face::Unmatched, false) => "rainbow-unmatched".into(),
        (Face::Mismatched, false) => "rainbow-mismatched".into(),
        (Face::Depth(n), true) => FALLBACK_DEPTH[(n.max(1) - 1) % FALLBACK_DEPTH.len()].into(),
        (Face::Unmatched, true) => "error".into(),
        (Face::Mismatched, true) => "warning".into(),
    }
}

/// Buffers the global mode applies to: files, but not plain text.
pub fn eligible(path: Option<&str>, major_mode: &str) -> bool {
    path.is_some() && !matches!(major_mode, "fundamental-mode" | "text-mode" | "org-mode")
}

fn pos(line: u32, column: u32) -> Position {
    Position { line, column }
}

fn is_on(b: &Buffer) -> bool {
    b.minor_modes().iter().any(|m| m == MODE)
}

/// Strings, comments and escapes, if the buffer has a grammar; None if it
/// hasn't, when all of it is taken to be code.
fn skips(b: &Buffer, text: &str) -> Option<Vec<Skip>> {
    let lines = text.split('\n').count() as u32;
    let all = Span {
        start: pos(0, 0),
        end: pos(lines, 0),
    };
    let faces = decorations::syntax(b, all);
    if faces.is_empty() {
        return None;
    }
    Some(
        faces
            .into_iter()
            .filter(|d| matches!(d.face.as_str(), "comment" | "string" | "escape"))
            .map(|d| {
                (
                    (d.span.start.line, d.span.start.column),
                    (d.span.end.line, d.span.end.column),
                )
            })
            .collect(),
    )
}

/// Colour a buffer's brackets. Returns how many.
fn check(b: &Buffer) -> Result<usize, String> {
    let text = b.text();
    if text.len() > MAX_BYTES {
        return Err(format!("{} is too big for rainbow delimiters", b.name()));
    }
    let skip = skips(b, &text);
    let found = delimiters::scan(&text, skip.as_deref().unwrap_or(&[]));
    let build = |fallback: bool| -> Vec<Decoration> {
        found
            .iter()
            .map(|d| Decoration {
                span: Span {
                    start: pos(d.line, d.column),
                    end: pos(d.line, d.column + 1),
                },
                face: face_name(
                    delimiters::pick_face(
                        d.depth,
                        d.matched,
                        MAX_FACE_COUNT,
                        OUTERMOST_ONLY_FACE_COUNT,
                    ),
                    fallback,
                ),
                message: None,
            })
            .collect()
    };
    let fallback = FALLBACK.get();
    match decorations::set(b, &build(fallback)) {
        Err(e) if !fallback && e.contains("unknown face") => {
            // An older Forge: nothing was set; try again with faces it has.
            FALLBACK.set(true);
            decorations::set(b, &build(true))?;
        }
        other => other?,
    }
    Ok(found.len())
}

fn enable(b: &Buffer) -> Result<(), String> {
    b.enable_minor_mode(MODE)?;
    check(b).map(|_| ())
}

fn disable(b: &Buffer) {
    b.disable_minor_mode(MODE);
    decorations::clear(b);
}

fn init() -> Result<(), String> {
    modes::define_minor_mode(MODE)?;
    modes::set_lighter(MODE, "")?;
    command(
        MODE,
        "Toggle colouring brackets by depth in this buffer. With a prefix argument, \
         a positive one turns it on and zero or a negative one turns it off.",
        |prefix| {
            let b = Buffer::current();
            let want = match prefix {
                None => !is_on(&b),
                Some(n) => n > 0,
            };
            if want {
                enable(&b)?;
                message("Rainbow delimiters on");
            } else {
                disable(&b);
                message("Rainbow delimiters off");
            }
            Ok(())
        },
    )?;

    command(
        "global-rainbow-delimiters-mode",
        "Toggle colouring brackets by depth in every file buffer (not plain text), now and when files are opened. \
         With a prefix argument, a positive one turns it on and zero or a negative one turns it off \
         (init.rhai: forge.command.run(\"global-rainbow-delimiters-mode\", 1)).",
        |prefix| {
            let on = match prefix {
                None => !GLOBAL.get(),
                Some(n) => n > 0,
            };
            GLOBAL.set(on);
            for b in Buffer::all() {
                if on && eligible(b.path().as_deref(), &b.major_mode()) {
                    enable(&b)?;
                } else if !on && is_on(&b) {
                    disable(&b);
                }
            }
            message(if on {
                "Global rainbow delimiters on"
            } else {
                "Global rainbow delimiters off"
            });
            Ok(())
        },
    )?;

    // Under Options, next to Show/Hide.
    extend_submenu(
        "Options",
        "Rainbow Delimiters",
        &[
            menu_item("This Buffer", MODE),
            menu_item("All Buffers", "global-rainbow-delimiters-mode"),
        ],
    )?;

    on(EventKind::BufferOpened, |e| {
        if let Event::BufferOpened(id) = e
            && GLOBAL.get()
            && let Some(b) = Buffer::from_id(*id)
            && eligible(b.path().as_deref(), &b.major_mode())
            && let Err(err) = enable(&b)
        {
            forge_plugin_sdk::warn(&err);
        }
    });
    // Recolour when typing pauses (Forge sends buffer-changed after idle).
    on(EventKind::BufferChanged, |e| {
        if let Event::BufferChanged(id) = e
            && let Some(b) = Buffer::from_id(*id).filter(is_on)
            && let Err(err) = check(&b)
        {
            forge_plugin_sdk::warn(&err);
        }
    });
    // Turned on or off some other way (init.rhai, another plugin).
    on(EventKind::ModeChanged, |e| {
        if let Event::ModeChanged(id) = e
            && let Some(b) = Buffer::from_id(*id)
        {
            if is_on(&b) {
                if let Err(err) = check(&b) {
                    forge_plugin_sdk::warn(&err);
                }
            } else {
                decorations::clear(&b);
            }
        }
    });
    Ok(())
}

forge_plugin!(init);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_names() {
        assert_eq!(face_name(Face::Depth(3), false), "rainbow-depth-3");
        assert_eq!(face_name(Face::Unmatched, false), "rainbow-unmatched");
        assert_eq!(face_name(Face::Mismatched, false), "rainbow-mismatched");
    }

    #[test]
    fn fallback_faces_exist_in_stock_forge() {
        // Names Forge already accepts, whatever the depth.
        let known = [
            "error",
            "warning",
            "keyword",
            "string",
            "function",
            "type",
            "constant",
            "builtin",
            "property",
            "attribute",
            "escape",
            "comment",
        ];
        for n in 1..=MAX_FACE_COUNT + 3 {
            assert!(known.contains(&face_name(Face::Depth(n), true).as_str()));
        }
        assert!(known.contains(&face_name(Face::Unmatched, true).as_str()));
        assert!(known.contains(&face_name(Face::Mismatched, true).as_str()));
    }

    #[test]
    fn global_mode_skips_prose_and_special_buffers() {
        assert!(eligible(Some("/a/b.rs"), "rust-mode"));
        assert!(!eligible(Some("/a/notes.txt"), "text-mode"));
        assert!(!eligible(Some("/a/x.org"), "org-mode"));
        assert!(!eligible(None, "rust-mode"), "*scratch*, *Help*, ...");
    }
}
