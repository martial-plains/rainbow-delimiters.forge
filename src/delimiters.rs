// SPDX-License-Identifier: GPL-3.0-or-later
//! The part of rainbow-delimiters that doesn't need an editor: find the
//! brackets in a text, work out how deeply each is nested and whether it
//! matches, and choose a face for it.
//!
//! Ported from Fanael Linithien's `rainbow-delimiters.el` (GPL-3.0-or-later),
//! which asks Emacs's syntax parser for the depth at each bracket. Here the
//! scan is done by hand, and strings and comments (which the parser would
//! have skipped) are passed in as spans to leave alone.

/// A position, as (line, column), both 0-based; columns count `char`s.
pub type Pos = (u32, u32);
/// Text to leave alone: start included, end not.
pub type Skip = (Pos, Pos);

/// One bracket that should be coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delim {
    /// Zero-based line containing the bracket.
    pub line: u32,

    /// Zero-based character column containing the bracket.
    ///
    /// Columns count Unicode scalar values (`char`s), not bytes or
    /// UTF-16 code units.
    pub column: u32,

    /// Nesting depth, with 1 representing the outermost pair.
    ///
    /// A closing bracket has the depth of the opening bracket it
    /// corresponds to. A depth of 0 means the closing bracket has
    /// no corresponding opening bracket.
    pub depth: usize,

    /// Whether the bracket is correctly matched.
    ///
    /// Closing brackets that have no corresponding opening bracket,
    /// or that conflict with an opening bracket of a different kind,
    /// are not matched.
    pub matched: bool,
}

/// Which face a bracket gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// 1-based depth face, 1 to `max_face_count`.
    Depth(usize),

    /// Face for a closing bracket with no corresponding opening bracket.
    ///
    /// For example, the `)` in `foo)`.
    Unmatched,

    /// Face for a closing bracket that conflicts with an opening bracket
    /// of a different kind.
    ///
    /// For example, the `]` in `(]`.
    Mismatched,
}

fn closer_of(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn is_closer(c: char) -> bool {
    matches!(c, ')' | ']' | '}')
}

/// Sort spans and merge the ones that touch or overlap, so a position can
/// be checked with a single forward walk.
fn merged(skip: &[Skip]) -> Vec<Skip> {
    let mut v: Vec<Skip> = skip.iter().copied().filter(|(s, e)| s < e).collect();
    v.sort();
    let mut out: Vec<Skip> = Vec::with_capacity(v.len());
    for (s, e) in v {
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Every `(`, `[`, `{`, `)`, `]` and `}` in `text` outside `skip`, with its
/// depth. Like Emacs's parser, any closing bracket ends the innermost open
/// one, whether or not the kinds agree; one with nothing open is unmatched
/// (depth 0) and doesn't disturb what follows.
pub fn scan(text: &str, skip: &[Skip]) -> Vec<Delim> {
    let skip = merged(skip);
    let mut next = 0; // first skip span that might still contain the position
    let mut stack: Vec<char> = Vec::new();
    let mut out = Vec::new();
    for (line, content) in text.split('\n').enumerate() {
        let line = line as u32;
        for (column, c) in content.chars().enumerate() {
            if !matches!(c, '(' | '[' | '{') && !is_closer(c) {
                continue;
            }
            let at = (line, column as u32);
            while next < skip.len() && skip[next].1 <= at {
                next += 1;
            }
            if next < skip.len() && skip[next].0 <= at {
                continue;
            }
            let (depth, matched) = match closer_of(c) {
                Some(_) => {
                    stack.push(c);
                    (stack.len(), true)
                }
                None => match stack.pop() {
                    Some(open) => (stack.len() + 1, closer_of(open) == Some(c)),
                    None => (0, false),
                },
            };
            out.push(Delim {
                line,
                column: at.1,
                depth,
                matched,
            });
        }
    }
    out
}

/// The face for a bracket: `rainbow-delimiters-default-pick-face`.
///
/// Deeper than `max_face_count`, the faces cycle, skipping the
/// `outermost_only` faces at the start, which are used for the outermost
/// levels only.
pub fn pick_face(
    depth: usize,
    matched: bool,
    max_face_count: usize,
    outermost_only: usize,
) -> Face {
    let max = max_face_count.max(1);
    let outer = outermost_only.min(max - 1);
    if depth == 0 {
        Face::Unmatched
    } else if !matched {
        Face::Mismatched
    } else if depth <= max {
        Face::Depth(depth)
    } else {
        Face::Depth(1 + outer + (depth - max - 1) % (max - outer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depths(text: &str, skip: &[Skip]) -> Vec<(usize, bool)> {
        scan(text, skip)
            .iter()
            .map(|d| (d.depth, d.matched))
            .collect()
    }

    #[test]
    fn nesting_depth() {
        // ( [ { } ] )
        assert_eq!(
            depths("([{}])", &[]),
            [
                (1, true),
                (2, true),
                (3, true),
                (3, true),
                (2, true),
                (1, true)
            ]
        );
        assert_eq!(
            depths("()()", &[]),
            [(1, true), (1, true), (1, true), (1, true)]
        );
    }

    #[test]
    fn unmatched_closer_is_depth_zero_and_harmless() {
        assert_eq!(depths(") ()", &[]), [(0, false), (1, true), (1, true)]);
    }

    #[test]
    fn unclosed_opener_still_counts() {
        assert_eq!(depths("((", &[]), [(1, true), (2, true)]);
    }

    #[test]
    fn mismatched_closer_ends_the_innermost_pair() {
        // "(]" : the ] mismatches; like Emacs's parser it still closes.
        assert_eq!(depths("(]", &[]), [(1, true), (1, false)]);
        // After it we're back at depth 0, so a following ) is unmatched.
        assert_eq!(depths("(])", &[]), [(1, true), (1, false), (0, false)]);
    }

    #[test]
    fn positions_count_chars_and_lines() {
        let d = scan("é(\n  [x]", &[]);
        assert_eq!((d[0].line, d[0].column), (0, 1));
        assert_eq!((d[1].line, d[1].column), (1, 2));
        assert_eq!((d[2].line, d[2].column), (1, 4));
    }

    #[test]
    fn skips_strings_and_comments() {
        // f("(", x) -- the ( in the string is ignored
        let text = "f(\"(\", x)";
        assert_eq!(depths(text, &[((0, 2), (0, 5))]), [(1, true), (1, true)]);
        // end is exclusive: a bracket right after the span counts
        assert_eq!(depths("\"a\"(", &[((0, 0), (0, 3))]), [(1, true)]);
        // a span across lines: the ) on line 1 is inside it, the one at
        // (2, 0) is just past its end
        assert_eq!(
            depths("(\n)\n)", &[((0, 1), (2, 0))]),
            [(1, true), (1, true)]
        );
    }

    #[test]
    fn overlapping_and_unsorted_skips() {
        let skip = [((0, 4), (0, 9)), ((0, 1), (0, 5)), ((0, 6), (0, 7))];
        // x(( ((  ))() : everything from column 1 to 8 is skipped
        assert_eq!(
            depths("x(( ((  ))()", &skip),
            [(0, false), (1, true), (1, true)]
        );
    }

    #[test]
    fn face_choice() {
        assert_eq!(pick_face(0, false, 9, 0), Face::Unmatched);
        assert_eq!(pick_face(3, false, 9, 0), Face::Mismatched);
        assert_eq!(pick_face(1, true, 9, 0), Face::Depth(1));
        assert_eq!(pick_face(9, true, 9, 0), Face::Depth(9));
        assert_eq!(pick_face(10, true, 9, 0), Face::Depth(1));
        assert_eq!(pick_face(18, true, 9, 0), Face::Depth(9));
        assert_eq!(pick_face(19, true, 9, 0), Face::Depth(1));
    }

    #[test]
    fn outermost_only_faces_are_not_cycled_back_to() {
        // Faces 1 and 2 are for the outermost two levels only.
        assert_eq!(pick_face(9, true, 9, 2), Face::Depth(9));
        assert_eq!(pick_face(10, true, 9, 2), Face::Depth(3));
        assert_eq!(pick_face(16, true, 9, 2), Face::Depth(9));
        assert_eq!(pick_face(17, true, 9, 2), Face::Depth(3));
        assert_eq!(pick_face(5, true, 3, 7), Face::Depth(3));
        assert_eq!(pick_face(2, true, 0, 0), Face::Depth(1));
    }
}
