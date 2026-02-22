//! Operator rewriting pass for pipeline (`|>`) and cons (`::`).
//!
//! Iteratively finds custom operators in expression context and rewrites
//! them to `__binop__` calls. Operators in strings, comments, and type
//! contexts are left untouched.

use sc_ast::ScSyntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Pipeline,
    Cons,
}

impl Op {
    fn precedence(self) -> u8 {
        match self {
            Op::Pipeline => 1,
            Op::Cons => 5,
        }
    }

    fn is_right_assoc(self) -> bool {
        match self {
            Op::Pipeline => false,
            Op::Cons => true,
        }
    }

    fn text(self) -> &'static str {
        match self {
            Op::Pipeline => "|>",
            Op::Cons => "::",
        }
    }
}

#[derive(Debug, Clone)]
struct OpOccurrence {
    op: Op,
    byte_start: usize,
    byte_end: usize,
}

/// Rewrite all custom operators in the source.
pub fn rewrite_operators(source: &str, syntax: &ScSyntax) -> String {
    let mut result = source.to_string();
    let mut iterations = 0;
    let max_iterations = 1000;

    loop {
        iterations += 1;
        if iterations > max_iterations {
            break;
        }

        let occurrences = find_operator_occurrences(&result, syntax);
        if occurrences.is_empty() {
            break;
        }

        // Select the next operator to process:
        // Highest precedence first. For same precedence, leftmost for left-assoc,
        // rightmost for right-assoc.
        let next = select_next_operator(&occurrences);

        let left = find_left_operand(&result, next.byte_start, next.op);
        let right = find_right_operand(&result, next.byte_end, next.op);

        let left_text = result[left..next.byte_start].trim();
        let right_text = result[next.byte_end..right].trim();
        let replacement = format!(
            "__binop__({}, \"{}\", {})",
            left_text,
            next.op.text(),
            right_text
        );

        result = format!("{}{}{}", &result[..left], replacement, &result[right..]);
    }

    result
}

fn find_operator_occurrences(source: &str, syntax: &ScSyntax) -> Vec<OpOccurrence> {
    let chars: Vec<char> = source.chars().collect();
    let mut occurrences = Vec::new();
    let mut i = 0;

    // Type context tracking
    let mut type_annotation_depth: i32 = 0;
    let mut angle_bracket_depth: i32 = 0;
    let mut in_type_alias = false;
    let mut in_interface = false;

    while i < chars.len() {
        // Skip strings, comments, template literals
        if let Some(skip) = skip_non_code(&chars, i) {
            i = skip;
            continue;
        }

        let byte_pos = char_offset_to_byte(&chars, i);

        // Track keywords
        if is_word_start(&chars, i) {
            let word_end = scan_word(&chars, i);
            let word: String = chars[i..word_end].iter().collect();

            match word.as_str() {
                "type" => {
                    in_type_alias = true;
                    type_annotation_depth = 0;
                }
                "interface" => {
                    in_interface = true;
                    type_annotation_depth = 0;
                }
                _ => {}
            }
            i = word_end;
            continue;
        }

        match chars[i] {
            ';' => {
                type_annotation_depth = 0;
                in_type_alias = false;
                in_interface = false;
            }
            ':' => {
                // Could be `::`  or type annotation `:`
                if i + 1 < chars.len() && chars[i + 1] == ':' {
                    // Potential `::` operator
                    if syntax.cons
                        && !in_type_context(
                            type_annotation_depth,
                            angle_bracket_depth,
                            in_type_alias,
                            in_interface,
                        )
                    {
                        let bs = byte_pos;
                        let be = char_offset_to_byte(&chars, i + 2);
                        occurrences.push(OpOccurrence {
                            op: Op::Cons,
                            byte_start: bs,
                            byte_end: be,
                        });
                    }
                    i += 2;
                    continue;
                } else {
                    // Type annotation colon - increment depth
                    type_annotation_depth += 1;
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    // Pipeline operator
                    if syntax.pipeline
                        && !in_type_context(
                            type_annotation_depth,
                            angle_bracket_depth,
                            in_type_alias,
                            in_interface,
                        )
                    {
                        let bs = byte_pos;
                        let be = char_offset_to_byte(&chars, i + 2);
                        occurrences.push(OpOccurrence {
                            op: Op::Pipeline,
                            byte_start: bs,
                            byte_end: be,
                        });
                    }
                    i += 2;
                    continue;
                }
            }
            '<' => {
                // Could be generic type parameter
                if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
                    angle_bracket_depth += 1;
                }
            }
            '>' => {
                if angle_bracket_depth > 0 {
                    angle_bracket_depth -= 1;
                }
            }
            '=' | ')' | '}' | ',' => {
                if !in_type_alias {
                    type_annotation_depth = type_annotation_depth.saturating_sub(1);
                }
            }
            '{' => {
                if in_interface {
                    // Don't reset inside interface body
                } else {
                    type_annotation_depth = 0;
                }
            }
            _ => {}
        }

        i += 1;
    }

    occurrences
}

fn in_type_context(
    type_depth: i32,
    angle_depth: i32,
    in_type_alias: bool,
    in_interface: bool,
) -> bool {
    type_depth > 0 || angle_depth > 0 || in_type_alias || in_interface
}

fn select_next_operator(occurrences: &[OpOccurrence]) -> &OpOccurrence {
    occurrences
        .iter()
        .max_by(|a, b| {
            let prec_cmp = a.op.precedence().cmp(&b.op.precedence());
            if prec_cmp != std::cmp::Ordering::Equal {
                return prec_cmp;
            }
            // Same precedence: right-assoc picks rightmost, left-assoc picks leftmost
            if a.op.is_right_assoc() {
                a.byte_start.cmp(&b.byte_start)
            } else {
                b.byte_start.cmp(&a.byte_start)
            }
        })
        .expect("select_next_operator called with empty occurrences")
}

fn find_left_operand(source: &str, op_start: usize, op: Op) -> usize {
    let chars: Vec<char> = source[..op_start].chars().collect();
    let mut i = chars.len();
    let mut depth: i32 = 0;

    // Skip trailing whitespace before the operator
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }

    let _content_end = i;

    while i > 0 {
        i -= 1;

        match chars[i] {
            ')' | ']' | '}' => depth += 1,
            '(' | '[' | '{' => {
                if depth == 0 {
                    return boundary_after(source, &chars, i + 1);
                }
                depth -= 1;
            }
            ';' | ',' if depth == 0 => {
                return boundary_after(source, &chars, i + 1);
            }
            '=' if depth == 0 => {
                // Don't match => (arrow)
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    continue;
                }
                // Don't match == or ===
                if i > 0 && chars[i - 1] == '=' {
                    continue;
                }
                return boundary_after(source, &chars, i + 1);
            }
            '>' if depth == 0 && i > 0 && chars[i - 1] == '|' => {
                if Op::Pipeline.precedence() <= op.precedence() {
                    return boundary_after(source, &chars, i + 1);
                }
            }
            ':' if depth == 0 => {
                if i > 0 && chars[i - 1] == ':' {
                    if Op::Cons.precedence() <= op.precedence() {
                        return boundary_after(source, &chars, i + 1);
                    }
                    // Skip the first ':' of '::'
                    i -= 1;
                } else {
                    return boundary_after(source, &chars, i + 1);
                }
            }
            _ => {}
        }
    }

    0
}

/// Return byte offset, skipping leading whitespace after a boundary token.
fn boundary_after(_source: &str, chars: &[char], pos: usize) -> usize {
    let mut p = pos;
    while p < chars.len() && chars[p].is_whitespace() {
        p += 1;
    }
    char_offset_to_byte(chars, p)
}

fn find_right_operand(source: &str, op_end: usize, op: Op) -> usize {
    let rest = &source[op_end..];
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    let mut depth: i32 = 0;

    while i < chars.len() {
        // Skip whitespace at the boundary
        if depth == 0 && chars[i].is_whitespace() && i == 0 {
            i += 1;
            continue;
        }

        match chars[i] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return op_end + char_offset_to_byte(&chars, i);
                }
                depth -= 1;
            }
            ';' if depth == 0 => {
                return op_end + char_offset_to_byte(&chars, i);
            }
            ',' if depth == 0 => {
                return op_end + char_offset_to_byte(&chars, i);
            }
            '|' if depth == 0 && i + 1 < chars.len() && chars[i + 1] == '>' => {
                if op.is_right_assoc() {
                    if Op::Pipeline.precedence() < op.precedence() {
                        return op_end + char_offset_to_byte(&chars, i);
                    }
                } else if Op::Pipeline.precedence() <= op.precedence() {
                    return op_end + char_offset_to_byte(&chars, i);
                }
            }
            ':' if depth == 0 && i + 1 < chars.len() && chars[i + 1] == ':' => {
                if op.is_right_assoc() {
                    if Op::Cons.precedence() < op.precedence() {
                        return op_end + char_offset_to_byte(&chars, i);
                    }
                } else if Op::Cons.precedence() <= op.precedence() {
                    return op_end + char_offset_to_byte(&chars, i);
                }
                // Skip the second `:` since we've checked `::`.
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    op_end + char_offset_to_byte(&chars, chars.len())
}

fn skip_non_code(chars: &[char], i: usize) -> Option<usize> {
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

    // String literals
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

    // Template literal (simplified)
    if chars[i] == '`' {
        let mut j = i + 1;
        while j < chars.len() && chars[j] != '`' {
            if chars[j] == '\\' {
                j += 1;
            }
            j += 1;
        }
        return Some(if j < chars.len() { j + 1 } else { j });
    }

    None
}

fn is_word_start(chars: &[char], i: usize) -> bool {
    if !chars[i].is_alphabetic() && chars[i] != '_' && chars[i] != '$' {
        return false;
    }
    if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '$') {
        return false;
    }
    true
}

fn scan_word(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
        i += 1;
    }
    i
}

fn char_offset_to_byte(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syntax_all() -> ScSyntax {
        ScSyntax::default()
    }

    #[test]
    fn pipeline_basic() {
        let input = "const x = a |> f;";
        let output = rewrite_operators(input, &syntax_all());
        assert_eq!(output, r#"const x = __binop__(a, "|>", f);"#);
    }

    #[test]
    fn pipeline_chained() {
        let input = "const x = a |> f |> g;";
        let output = rewrite_operators(input, &syntax_all());
        assert_eq!(
            output,
            r#"const x = __binop__(__binop__(a, "|>", f), "|>", g);"#
        );
    }

    #[test]
    fn cons_basic() {
        let input = "const x = 1 :: [];";
        let output = rewrite_operators(input, &syntax_all());
        assert_eq!(output, r#"const x = __binop__(1, "::", []);"#);
    }

    #[test]
    fn cons_chained() {
        let input = "const x = 1 :: 2 :: [];";
        let output = rewrite_operators(input, &syntax_all());
        assert_eq!(
            output,
            r#"const x = __binop__(1, "::", __binop__(2, "::", []));"#
        );
    }

    #[test]
    fn pipeline_in_string_not_rewritten() {
        let input = r#"const s = "a |> b";"#;
        let output = rewrite_operators(input, &syntax_all());
        assert_eq!(output, input);
    }

    #[test]
    fn cons_and_pipeline_mixed() {
        let input = "const x = a :: b |> f;";
        let output = rewrite_operators(input, &syntax_all());
        // :: binds tighter than |>
        assert_eq!(
            output,
            r#"const x = __binop__(__binop__(a, "::", b), "|>", f);"#
        );
    }
}
