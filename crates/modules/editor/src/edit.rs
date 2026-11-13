//! Commands for text editing, clipboard, undo and the lightweight symbol
//! actions. Everything acts on the open buffer; none of it reads other files.

use crate::commands;
use crate::model;
use crate::state::{Completion, ParameterHint, RenameState, Shared};
use crate::symbols;
use forja_kernel::{Registrar, SettingValue};
use serde_json::{json, Value};

pub fn register(r: &mut Registrar<'_>, shared: &Shared) {
    register_history(r, shared);
    register_clipboard(r, shared);
    register_symbols(r, shared);
    register_fixes(r, shared);
}

fn register_history(r: &mut Registrar<'_>, shared: &Shared) {
    let undo = shared.clone();
    r.command("editor.undo", "editor.command.undo", move |cx, args| {
        let path = commands::active_path(cx, &undo, &args)?;
        let changed = undo.undo(&path);
        undo.set_status(Some(cx.t(if changed {
            "editor.status.undo"
        } else {
            "editor.status.undo-empty"
        })));
        Ok(json!({ "undone": changed }))
    });

    let redo = shared.clone();
    r.command("editor.redo", "editor.command.redo", move |cx, args| {
        let path = commands::active_path(cx, &redo, &args)?;
        let changed = redo.redo(&path);
        redo.set_status(Some(cx.t(if changed {
            "editor.status.redo"
        } else {
            "editor.status.redo-empty"
        })));
        Ok(json!({ "redone": changed }))
    });
}

fn register_clipboard(r: &mut Registrar<'_>, shared: &Shared) {
    let copy = shared.clone();
    r.command("editor.copy", "editor.command.copy", move |cx, args| {
        let path = commands::active_path(cx, &copy, &args)?;
        let Some(tab) = copy.tab(&path) else {
            return Ok(json!({ "copied": 0 }));
        };
        let Some((start, end)) = copy.selection() else {
            copy.set_status(Some(cx.t("editor.status.no-selection")));
            return Ok(json!({ "copied": 0 }));
        };
        let Some(context) = copy.ui_context() else {
            copy.set_status(Some(cx.t("editor.status.copy-unavailable")));
            return Ok(json!({ "copied": 0 }));
        };
        let text = model::slice_chars(&tab.content, start, end).to_owned();
        context.copy_text(text.clone());
        copy.set_status(Some(cx.t("editor.status.copied")));
        Ok(json!({ "copied": text.chars().count() }))
    });

    let cut = shared.clone();
    r.command("editor.cut", "editor.command.cut", move |cx, args| {
        let path = commands::active_path(cx, &cut, &args)?;
        let Some(tab) = cut.tab(&path) else {
            return Ok(json!({ "cut": 0 }));
        };
        let Some((start, end)) = cut.selection() else {
            cut.set_status(Some(cx.t("editor.status.no-selection")));
            return Ok(json!({ "cut": 0 }));
        };
        let Some(context) = cut.ui_context() else {
            cut.set_status(Some(cx.t("editor.status.copy-unavailable")));
            return Ok(json!({ "cut": 0 }));
        };
        let text = model::slice_chars(&tab.content, start, end).to_owned();
        context.copy_text(text.clone());
        let from = model::byte_for_char(&tab.content, start);
        let to = model::byte_for_char(&tab.content, end);
        let mut next = String::with_capacity(tab.content.len() - (to - from));
        next.push_str(&tab.content[..from]);
        next.push_str(&tab.content[to..]);
        cut.record_undo(&path, tab.content);
        cut.set_content(&path, next);
        cut.with(|state| state.selection = None);
        cut.set_status(Some(cx.t("editor.status.cut")));
        Ok(json!({ "cut": text.chars().count() }))
    });

    let paste = shared.clone();
    r.command("editor.paste", "editor.command.paste", move |cx, _args| {
        if let Some(context) = paste.ui_context() {
            context.send_viewport_cmd(forja_kernel::ui_contract::egui::ViewportCommand::RequestPaste);
            Ok(json!({ "requested": true }))
        } else {
            paste.set_status(Some(cx.t("editor.status.paste-unavailable")));
            Ok(json!({ "requested": false }))
        }
    });

    let select_all = shared.clone();
    r.command(
        "editor.select_all",
        "editor.command.select_all",
        move |cx, args| {
            let path = commands::active_path(cx, &select_all, &args)?;
            let chars = select_all
                .tab(&path)
                .map(|tab| tab.content.chars().count())
                .unwrap_or(0);
            select_all.with(|state| state.pending_select_all = true);
            Ok(json!({ "selected": chars }))
        },
    );
}

fn register_symbols(r: &mut Registrar<'_>, shared: &Shared) {
    let goto = shared.clone();
    r.command(
        "editor.goto.definition",
        "editor.command.goto_definition",
        move |cx, args| {
            let path = commands::active_path(cx, &goto, &args)?;
            let Some(tab) = goto.tab(&path) else {
                return Ok(json!({ "line": Value::Null }));
            };
            let cursor = goto.cursor_char();
            let Some((word, _)) = symbols::word_at(&tab.content, cursor) else {
                goto.set_status(Some(cx.t("editor.status.no-symbol")));
                return Ok(json!({ "line": Value::Null }));
            };
            match symbols::definition_line(&tab.content, &word) {
                Some(line) => {
                    goto.with(|state| state.pending_line = Some(line));
                    goto.set_status(Some(cx.t_args(
                        "editor.status.definition",
                        &[("word", &word), ("line", &line.to_string())],
                    )));
                    Ok(json!({ "line": line, "word": word }))
                }
                None => {
                    goto.set_status(Some(cx.t_args(
                        "editor.status.definition-missing",
                        &[("word", &word)],
                    )));
                    Ok(json!({ "line": Value::Null, "word": word }))
                }
            }
        },
    );

    let peek = shared.clone();
    r.command(
        "editor.peek.definition",
        "editor.command.peek_definition",
        move |cx, args| {
            let path = commands::active_path(cx, &peek, &args)?;
            let Some(tab) = peek.tab(&path) else {
                return Ok(json!({ "peek": false }));
            };
            let cursor = peek.cursor_char();
            let Some((word, _)) = symbols::word_at(&tab.content, cursor) else {
                peek.set_status(Some(cx.t("editor.status.no-symbol")));
                return Ok(json!({ "peek": false }));
            };
            let Some((line, text)) = symbols::signature_line(&tab.content, &word) else {
                peek.set_status(Some(cx.t_args(
                    "editor.status.definition-missing",
                    &[("word", &word)],
                )));
                return Ok(json!({ "peek": false }));
            };
            peek.with(|state| state.peek = Some(crate::state::Peek { word, line, text }));
            Ok(json!({ "peek": true, "line": line }))
        },
    );

    let rename = shared.clone();
    r.command("editor.rename", "editor.command.rename", move |cx, args| {
        let path = commands::active_path(cx, &rename, &args)?;
        let Some(tab) = rename.tab(&path) else {
            return Ok(json!({ "renamed": 0 }));
        };
        let word = args
            .str("word")
            .map(str::to_owned)
            .or_else(|| symbols::word_at(&tab.content, rename.cursor_char()).map(|(word, _)| word));
        let Some(word) = word else {
            rename.set_status(Some(cx.t("editor.status.no-symbol")));
            return Ok(json!({ "renamed": 0 }));
        };
        let Some(replacement) = args.str("replacement").map(str::to_owned) else {
            rename.with(|state| {
                state.rename = Some(RenameState {
                    text: word.clone(),
                    word,
                })
            });
            return Ok(json!({ "prompt": true }));
        };
        let case_sensitive = cx
            .setting("editor.search.case_sensitive")
            .and_then(SettingValue::as_bool)
            .unwrap_or(false);
        let (next, count) = symbols::rename(&tab.content, &word, &replacement, case_sensitive);
        if count > 0 {
            rename.record_undo(&path, tab.content);
            rename.set_content(&path, next);
        }
        rename.with(|state| state.rename = None);
        rename.set_status(Some(cx.t_args(
            "editor.status.renamed",
            &[("count", &count.to_string())],
        )));
        Ok(json!({ "renamed": count }))
    });

    let completion = shared.clone();
    r.command(
        "editor.completion",
        "editor.command.completion",
        move |cx, args| {
            let path = commands::active_path(cx, &completion, &args)?;
            let Some(tab) = completion.tab(&path) else {
                return Ok(json!({ "candidates": 0 }));
            };
            let cursor = completion.cursor_char();
            let (prefix, range) = symbols::prefix_at(&tab.content, cursor);
            let candidates = symbols::completions(&tab.content, &prefix, 20);
            if candidates.is_empty() {
                completion.set_status(Some(cx.t("editor.status.no-completions")));
                completion.with(|state| state.completion = None);
                return Ok(json!({ "candidates": 0 }));
            }
            let count = candidates.len();
            completion.with(|state| {
                state.completion = Some(Completion {
                    start: range.start,
                    end: range.end,
                    candidates,
                })
            });
            Ok(json!({ "candidates": count }))
        },
    );

    let apply_completion = shared.clone();
    r.command(
        "editor.completion.apply",
        "editor.command.completion_apply",
        move |cx, args| {
            let path = commands::active_path(cx, &apply_completion, &args)?;
            let Some(tab) = apply_completion.tab(&path) else {
                return Ok(json!({ "applied": false }));
            };
            let Some(word) = args.str("word").map(str::to_owned) else {
                return Ok(json!({ "applied": false }));
            };
            let Some(completion) = apply_completion.with(|state| state.completion.clone()).flatten()
            else {
                return Ok(json!({ "applied": false }));
            };
            let from = model::byte_for_char(&tab.content, completion.start);
            let to = model::byte_for_char(&tab.content, completion.end);
            let mut next = String::with_capacity(tab.content.len() + word.len());
            next.push_str(&tab.content[..from]);
            next.push_str(&word);
            next.push_str(&tab.content[to..]);
            apply_completion.record_undo(&path, tab.content);
            apply_completion.set_content(&path, next);
            apply_completion.with(|state| {
                state.completion = None;
                state.cursor_char = completion.start + word.chars().count();
            });
            Ok(json!({ "applied": true }))
        },
    );

    let hint = shared.clone();
    r.command(
        "editor.parameter_hint",
        "editor.command.parameter_hint",
        move |cx, args| {
            let path = commands::active_path(cx, &hint, &args)?;
            let Some(tab) = hint.tab(&path) else {
                return Ok(json!({ "hint": false }));
            };
            let Some(word) = symbols::call_word_at(&tab.content, hint.cursor_char()) else {
                hint.set_status(Some(cx.t("editor.status.no-call")));
                return Ok(json!({ "hint": false }));
            };
            match symbols::signature_line(&tab.content, &word) {
                Some((line, text)) => {
                    hint.with(|state| state.parameter_hint = Some(ParameterHint { line, text }));
                    Ok(json!({ "hint": true, "line": line }))
                }
                None => {
                    hint.set_status(Some(cx.t_args(
                        "editor.status.definition-missing",
                        &[("word", &word)],
                    )));
                    Ok(json!({ "hint": false }))
                }
            }
        },
    );
}

fn register_fixes(r: &mut Registrar<'_>, shared: &Shared) {
    let open = shared.clone();
    r.command("editor.quick_fix", "editor.command.quick_fix", move |cx, args| {
        let _ = commands::active_path(cx, &open, &args)?;
        open.with(|state| state.quick_fix = true);
        Ok(json!({ "open": true }))
    });

    let apply = shared.clone();
    r.command(
        "editor.quick_fix.apply",
        "editor.command.quick_fix_apply",
        move |cx, args| {
            let path = commands::active_path(cx, &apply, &args)?;
            let Some(tab) = apply.tab(&path) else {
                return Ok(json!({ "applied": false }));
            };
            let fix = args.str("fix").unwrap_or("trim");
            let size = cx
                .setting("editor.indent.size")
                .and_then(SettingValue::as_i64)
                .unwrap_or(4)
                .clamp(1, 8) as usize;
            let next = match fix {
                "newline" => symbols::ensure_final_newline(&tab.content),
                "spaces" => symbols::tabs_to_spaces(&tab.content, size),
                _ => symbols::trim_trailing(&tab.content),
            };
            if next != tab.content {
                apply.record_undo(&path, tab.content);
                apply.set_content(&path, next);
            }
            apply.with(|state| state.quick_fix = false);
            apply.set_status(Some(cx.t("editor.status.quick-fixed")));
            Ok(json!({ "applied": true, "fix": fix }))
        },
    );
}

/// Runs a fix from the view without going through the command registry.
pub fn quick_fix_label_key(fix: &str) -> &'static str {
    match fix {
        "newline" => "editor.quick-fix.newline",
        "spaces" => "editor.quick-fix.spaces",
        _ => "editor.quick-fix.trim",
    }
}
