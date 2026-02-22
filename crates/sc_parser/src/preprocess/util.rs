//! Shared utilities for preprocessing passes.

/// Skip over non-code regions: comments and strings (NOT template literals).
///
/// Template literals require special handling because they contain `${...}`
/// interpolations with real code that needs processing. Callers must handle
/// template literals separately using `TemplateState`.
///
/// Returns `Some(new_position)` if `i` is at the start of a non-code region,
/// where `new_position` is the first character after the region.
/// Returns `None` if `i` is not at the start of a non-code region.
pub(super) fn skip_non_code(chars: &[char], i: usize) -> Option<usize> {
    if i >= chars.len() {
        return Some(chars.len());
    }

    // Single-line comment
    if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
        let mut j = i + 2;
        while j < chars.len() && chars[j] != '\n' {
            j += 1;
        }
        return Some(j + 1);
    }

    // Multi-line comment
    if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
        let mut j = i + 2;
        while j + 1 < chars.len() {
            if chars[j] == '*' && chars[j + 1] == '/' {
                return Some(j + 2);
            }
            j += 1;
        }
        return Some(chars.len());
    }

    // String literals (NOT template literals - those need special handling)
    if chars[i] == '"' || chars[i] == '\'' {
        let quote = chars[i];
        let mut j = i + 1;
        while j < chars.len() && chars[j] != quote {
            if chars[j] == '\\' {
                j += 1;
            }
            j += 1;
        }
        return Some(if j < chars.len() { j + 1 } else { j });
    }

    None
}

/// State for tracking template literal nesting.
///
/// Each entry in the stack represents a template literal, with the value being
/// the brace depth within that literal's current interpolation:
/// - `0` = in the literal part (between `` ` `` and `${`, or between `}` and `${`/`` ` ``)
/// - `> 0` = inside an interpolation with that brace depth
#[derive(Default)]
pub(super) struct TemplateState {
    stack: Vec<i32>,
}

impl TemplateState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if we're currently inside a template literal (either literal part or interpolation).
    #[allow(dead_code)]
    pub fn in_template(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Check if we're in the literal part of a template (should skip this character).
    #[allow(dead_code)]
    pub fn in_literal_part(&self) -> bool {
        self.stack.last().is_some_and(|&d| d == 0)
    }

    /// Handle a character and return whether it was consumed by template handling.
    /// If true, the caller should continue to the next character.
    /// If false, the caller should process this character normally.
    pub fn handle_char(&mut self, chars: &[char], i: usize) -> HandleResult {
        if self.stack.is_empty() {
            // Not in a template literal - check for start
            if chars[i] == '`' {
                self.stack.push(0); // Start in literal part
                return HandleResult::Skip(1);
            }
            return HandleResult::Process;
        }

        let depth = *self.stack.last().unwrap();

        if depth == 0 {
            // In literal part - skip until ${ or closing `
            if chars[i] == '\\' && i + 1 < chars.len() {
                return HandleResult::Skip(2);
            }
            if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                // Start of interpolation
                *self.stack.last_mut().unwrap() = 1;
                return HandleResult::Skip(2);
            }
            if chars[i] == '`' {
                // End of this template literal
                self.stack.pop();
                return HandleResult::Skip(1);
            }
            // Skip literal character
            return HandleResult::Skip(1);
        }

        // In interpolation - track braces and nested templates
        if chars[i] == '`' {
            // Nested template literal
            self.stack.push(0);
            return HandleResult::Skip(1);
        }

        if chars[i] == '{' {
            *self.stack.last_mut().unwrap() += 1;
            // Still process this character (could be part of an object literal in code)
            return HandleResult::Process;
        }

        if chars[i] == '}' {
            let d = self.stack.last_mut().unwrap();
            *d -= 1;
            if *d == 0 {
                // End of interpolation, back to literal part
                return HandleResult::Skip(1);
            }
            // Still in nested braces, process normally
            return HandleResult::Process;
        }

        // Normal character inside interpolation - process it
        HandleResult::Process
    }
}

/// Result of handling a character with TemplateState.
pub(super) enum HandleResult {
    /// Skip this many characters (template handling consumed them).
    Skip(usize),
    /// Process this character normally (it's code, not template literal content).
    Process,
}

/// Convert a character index to a byte offset in UTF-8.
pub(super) fn char_offset_to_byte(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}
