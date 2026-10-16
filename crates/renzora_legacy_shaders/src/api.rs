//! Bevy shader functions whose signature changed in 0.20, rewritten at the
//! call site when the change is mechanical.
//!
//! Most API changes cannot be fixed by text (a removed struct field has no
//! replacement to write), and those surface as compile errors. A few are a
//! different spelling of the same call, and those are worth fixing here:
//! every `.material` saved before 0.20 embeds a shader that calls them.
//!
//! The rewrite edits token text in place, never the number of tokens, so the
//! event stream the rest of the translator indexes stays valid.

use crate::lex::{Kind, Token};
use crate::Diagnostic;

/// `alpha_discard(material, color)` became `alpha_discard(flags, cutoff,
/// color)`: 0.20 takes the two fields it reads instead of the whole
/// `StandardMaterial`. A call with two arguments is the old form.
pub fn rewrite_calls(tokens: &mut [Token], lines: &[usize], diagnostics: &mut Vec<Diagnostic>) {
    // Path rewriting folds a qualified path into its first token and empties
    // the rest, so an emptied token is skipped like trivia and the name may
    // arrive fully qualified.
    let sig: Vec<usize> = (0..tokens.len())
        .filter(|&i| tokens[i].kind != Kind::Trivia && !tokens[i].text.is_empty())
        .collect();
    for k in 0..sig.len() {
        let name = sig[k];
        let text = &tokens[name].text;
        let is_call = tokens[name].kind == Kind::Ident
            && (text == "alpha_discard" || text.ends_with("::alpha_discard"));
        if !is_call {
            continue;
        }
        if !sig.get(k + 1).is_some_and(|&j| tokens[j].is("(")) {
            continue;
        }
        let Some((commas, close)) = top_level_commas(tokens, &sig, k + 1) else { continue };
        if commas.len() != 1 {
            continue;
        }
        let comma = commas[0];
        let open = sig[k + 1];
        let material: String = tokens[open + 1..comma].iter().map(|t| t.text.as_str()).collect();
        let material = material.trim().to_string();
        if material.is_empty() || close <= comma + 1 {
            continue;
        }
        tokens[comma].text = format!(".flags, {material}.alpha_cutoff,");
        diagnostics.push(Diagnostic::info(
            lines.get(name).copied().unwrap_or(0),
            "`alpha_discard` takes the material's flags and alpha cutoff since Bevy 0.20; \
             the call was rewritten to pass them",
        ));
    }
}

/// The token indices of the commas directly inside the parenthesis at
/// `sig[open_k]`, and the index of its closing parenthesis.
fn top_level_commas(tokens: &[Token], sig: &[usize], open_k: usize) -> Option<(Vec<usize>, usize)> {
    let mut depth = 0i32;
    let mut commas = Vec::new();
    for &i in &sig[open_k..] {
        match tokens[i].text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some((commas, i));
                }
            }
            "," if depth == 1 => commas.push(i),
            _ => {}
        }
    }
    None
}
