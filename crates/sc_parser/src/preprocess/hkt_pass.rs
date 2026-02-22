//! HKT type parameter rewriting pass.
//!
//! Finds `F<_>` declarations in type parameter lists, strips `<_>`, and
//! rewrites usages of `F<A>` to `$<F, A>` within the declaring scope.

use super::util::{char_offset_to_byte, skip_non_code, HandleResult, TemplateState};

#[derive(Debug, Clone)]
struct HktDecl {
    name: String,
    /// Range of `<_>` to remove (byte offsets into the original source).
    remove_start: usize,
    remove_end: usize,
    /// Scope in which `F<A>` → `$<F, A>` applies.
    scope_start: usize,
    scope_end: usize,
}

#[derive(Debug, Clone)]
struct HktUsage {
    /// Byte offset of the identifier (e.g. `F`).
    ident_start: usize,
    /// Byte offset after the closing `>`.
    end: usize,
    /// The parameter name.
    name: String,
    /// The inner args text (between `<` and `>`).
    args: String,
}

/// Rewrite all HKT syntax in the source.
pub fn rewrite_hkt(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();

    let decls = find_hkt_declarations(&chars, source);
    if decls.is_empty() {
        return source.to_string();
    }

    let usages = find_hkt_usages(&chars, source, &decls);

    apply_hkt_replacements(source, &decls, &usages)
}

fn find_hkt_declarations(chars: &[char], source: &str) -> Vec<HktDecl> {
    let mut decls = Vec::new();
    let mut i = 0;
    let mut template_state = TemplateState::new();

    while i < chars.len() {
        // Handle template literals (process code in interpolations, skip literal parts)
        match template_state.handle_char(chars, i) {
            HandleResult::Skip(n) => {
                i += n;
                continue;
            }
            HandleResult::Process => {}
        }

        // Skip strings and comments.
        if let Some(skip) = skip_non_code(chars, i) {
            i = skip;
            continue;
        }

        // Look for uppercase identifier followed by `<_>`
        if chars[i].is_ascii_uppercase() {
            let ident_start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[ident_start..i].iter().collect();

            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }

            if i < chars.len() && chars[i] == '<' {
                let angle_byte_start = char_offset_to_byte(chars, i);
                i += 1;

                // Skip whitespace
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }

                // Check for `_` (possibly comma-separated)
                let mut all_underscores = true;

                if i < chars.len() && chars[i] == '_' {
                    i += 1;

                    // Check for more `_, _` patterns
                    loop {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == ',' {
                            i += 1;
                            while i < chars.len() && chars[i].is_whitespace() {
                                i += 1;
                            }
                            if i < chars.len() && chars[i] == '_' {
                                i += 1;
                            } else {
                                all_underscores = false;
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    // Skip whitespace
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }

                    if all_underscores && i < chars.len() && chars[i] == '>' {
                        i += 1;
                        let angle_byte_end = char_offset_to_byte(chars, i);

                        let scope = find_enclosing_scope(chars, ident_start, source);

                        decls.push(HktDecl {
                            name,
                            remove_start: angle_byte_start,
                            remove_end: angle_byte_end,
                            scope_start: scope.0,
                            scope_end: scope.1,
                        });
                        continue;
                    }
                }
            }
            continue;
        }

        i += 1;
    }

    decls
}

fn find_hkt_usages(chars: &[char], _source: &str, decls: &[HktDecl]) -> Vec<HktUsage> {
    let mut usages = Vec::new();
    let mut i = 0;
    let mut template_state = TemplateState::new();

    while i < chars.len() {
        // Handle template literals (process code in interpolations, skip literal parts)
        match template_state.handle_char(chars, i) {
            HandleResult::Skip(n) => {
                i += n;
                continue;
            }
            HandleResult::Process => {}
        }

        if let Some(skip) = skip_non_code(chars, i) {
            i = skip;
            continue;
        }

        if chars[i].is_ascii_uppercase() {
            let ident_start = i;
            let ident_byte_start = char_offset_to_byte(chars, i);
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[ident_start..i].iter().collect();

            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }

            if i < chars.len() && chars[i] == '<' {
                if let Some(close) = find_matching_angle(chars, i) {
                    let inner_start = i + 1;
                    let inner_chars: String = chars[inner_start..close].iter().collect();

                    // Check it's not just underscores (that's a declaration, not usage)
                    let trimmed = inner_chars.trim();
                    if trimmed == "_"
                        || trimmed
                            .chars()
                            .all(|c| c == '_' || c == ',' || c.is_whitespace())
                    {
                        i = close + 1;
                        continue;
                    }

                    let usage_byte_start = ident_byte_start;
                    let usage_byte_end = char_offset_to_byte(chars, close + 1);

                    // Check if this usage is within any HKT declaration's scope
                    if find_active_decl(decls, &name, usage_byte_start).is_some() {
                        usages.push(HktUsage {
                            ident_start: usage_byte_start,
                            end: usage_byte_end,
                            name: name.clone(),
                            args: inner_chars.trim().to_string(),
                        });
                    }

                    i = close + 1;
                    continue;
                }
            }
            continue;
        }

        i += 1;
    }

    usages
}

fn find_active_decl<'a>(decls: &'a [HktDecl], name: &str, pos: usize) -> Option<&'a HktDecl> {
    decls
        .iter()
        .filter(|d| d.name == name && pos >= d.scope_start && pos <= d.scope_end)
        .min_by_key(|d| d.scope_end - d.scope_start)
}

fn apply_hkt_replacements(source: &str, decls: &[HktDecl], usages: &[HktUsage]) -> String {
    // Collect all replacements sorted by position (descending for safe replacement).
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();

    for decl in decls {
        replacements.push((decl.remove_start, decl.remove_end, String::new()));
    }

    for usage in usages {
        let replacement = format!("$<{}, {}>", usage.name, usage.args);
        replacements.push((usage.ident_start, usage.end, replacement));
    }

    // Sort by start position descending so replacements don't shift offsets.
    replacements.sort_by(|a, b| b.0.cmp(&a.0));

    // Remove overlapping replacements (keep the first = outermost by position).
    let mut filtered = Vec::new();
    for r in &replacements {
        if filtered
            .iter()
            .any(|(s, e, _): &(usize, usize, String)| r.0 >= *s && r.1 <= *e)
        {
            continue;
        }
        filtered.push(r.clone());
    }

    let mut result = source.to_string();
    for (start, end, replacement) in filtered {
        result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
    }

    result
}

fn find_enclosing_scope(chars: &[char], pos: usize, source: &str) -> (usize, usize) {
    // The HKT declaration is in a type parameter list (e.g. `interface Foo<F<_>> { ... }`).
    // The scope includes everything from the declaration's container start through
    // the closing `}` or `;`.

    // Scan backward to find the start of the containing declaration.
    let mut scope_start_char = 0;
    let mut j = pos;
    while j > 0 {
        j -= 1;
        // Stop at statement-ending tokens
        if chars[j] == '}' || chars[j] == ';' {
            scope_start_char = j + 1;
            break;
        }
    }
    let scope_start = char_offset_to_byte(chars, scope_start_char);

    // Scan forward from the declaration to find the end of the scope.
    // Look for the matching closing `}` or `;` at depth 0.
    let mut scope_end = source.len();
    let mut depth = 0;
    let mut j = pos;
    let mut template_state = TemplateState::new();

    while j < chars.len() {
        // Handle template literals
        match template_state.handle_char(chars, j) {
            HandleResult::Skip(n) => {
                j += n;
                continue;
            }
            HandleResult::Process => {}
        }

        if let Some(skip) = skip_non_code(chars, j) {
            j = skip;
            continue;
        }
        match chars[j] {
            '{' => depth += 1,
            '}' => {
                if depth <= 1 {
                    scope_end = char_offset_to_byte(chars, j + 1);
                    break;
                }
                depth -= 1;
            }
            ';' if depth == 0 => {
                scope_end = char_offset_to_byte(chars, j + 1);
                break;
            }
            _ => {}
        }
        j += 1;
    }

    (scope_start, scope_end)
}

fn find_matching_angle(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            // Don't cross statement boundaries
            ';' | '{' | '}' if depth <= 1 => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkt_basic_declaration() {
        let input = "interface Functor<F<_>> {\n  map: <A, B>(fa: F<A>) => F<B>;\n}";
        let output = rewrite_hkt(input);
        assert!(output.contains("Functor<F>"), "Should strip <_>: {output}");
        assert!(output.contains("$<F, A>"), "Should rewrite F<A>: {output}");
        assert!(output.contains("$<F, B>"), "Should rewrite F<B>: {output}");
    }

    #[test]
    fn hkt_no_rewrite_outside_scope() {
        let input =
            "interface Functor<F<_>> { map: (fa: F<A>) => F<B>; }\nconst x: F<number> = foo;";
        let output = rewrite_hkt(input);
        // F<number> outside the interface scope should NOT be rewritten
        assert!(
            output.contains("F<number>"),
            "Should not rewrite outside scope: {output}"
        );
    }
}
