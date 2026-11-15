//! Comandos do módulo terminal (Bigorna, atalhos e CLI).

use crate::core::{CloseTarget, OpenRequest};
use crate::panes::tree::{SplitDir, MAX_PANES};
use crate::requests::{open_request, parse_id};
use crate::settings::TermSettings;
use crate::Shared;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, KernelError, Registrar};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub(super) fn register_commands(r: &mut Registrar<'_>, core: &Shared) {
    let recipe_core = Arc::clone(core);
    r.command("terminal.recipe.run", "terminal.command.recipe-run", move |cx, args| {
        let id = args.required_str("recipe_id")?;
        let hash = args.required_str("content_hash")?;
        let bench = recipe_bench(cx);
        let bench_text = bench.as_ref().map(|path| path.to_string_lossy().into_owned());
        if args.str("bench_path") != bench_text.as_deref() {
            return Err(KernelError::Message(cx.t("terminal.view.recipe-stale")));
        }
        let recipe = recipe_core.with_hub(|hub| {
            hub.recipes.iter().find(|recipe| recipe.id == id && recipe.content_hash == hash).cloned()
        }).map_err(command_error)?
            .ok_or_else(|| KernelError::Message(cx.t("terminal.view.recipe-stale")))?;
        crate::recipe_trust::approve(cx, &recipe).map_err(command_error)?;
        let settings = TermSettings::from_ctx(cx);
        let worker = Arc::clone(&recipe_core);
        let completion = Arc::clone(&recipe_core);
        cx.spawn(move || worker.with_hub(|hub| {
            if bench.is_some() && hub.bench_path != bench {
                hub.set_bench(bench.clone());
            }
            if hub.focused_terminal().is_none() && settings.recipe_program().is_none() {
                hub.open(&settings, OpenRequest { cwd: bench, ..OpenRequest::default() })?;
            }
            hub.run_recipe(&recipe, &settings)
        }).and_then(|result| result), move |cx, result| {
            if result.is_err() {
                let notice = cx.t("terminal.view.recipe-failed");
                let _ = completion.with_hub(|hub| hub.notice = Some(notice));
            }
        });
        Ok(json!({ "pending": true }))
    });

    let open_core = Arc::clone(core);
    r.command("terminal.open", "terminal.command.open", move |cx, args| {
        let settings = TermSettings::from_ctx(cx);
        let request = open_request(&args);
        let id = open_core
            .with_hub(|hub| hub.open(&settings, request))
            .map_err(command_error)?
            .map_err(command_error)?;
        Ok(json!({ "terminal_id": id.to_string() }))
    });

    let send_core = Arc::clone(core);
    r.command("terminal.send", "terminal.command.send", move |_cx, args| {
        let text = args.required_str("text")?.to_owned();
        let enter = args.bool("enter").unwrap_or(true);
        let target = args.str("terminal_id").and_then(parse_id);
        send_core
            .with_hub(|hub| {
                let terminal = match target {
                    Some(id) => hub.terminal_mut(id),
                    None => hub.focused_terminal_mut(),
                }
                .ok_or_else(|| "terminal not found".to_owned())?;
                terminal.send(&text, enter);
                Ok::<_, String>(json!({}))
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    let read_core = Arc::clone(core);
    r.command("terminal.read", "terminal.command.read", move |cx, args| {
        let settings = TermSettings::from_ctx(cx);
        let target = args.str("terminal_id").and_then(parse_id);
        let lines = args.i64("lines").unwrap_or(200).clamp(1, 5_000) as usize;
        read_core
            .with_hub(|hub| {
                let context = hub.ui_context.clone();
                hub.poll(Instant::now(), &settings, context.as_ref());
                let terminal = match target {
                    Some(id) => hub.terminal(id),
                    None => hub.focused_terminal(),
                }
                .ok_or_else(|| "terminal not found".to_owned())?;
                Ok::<_, String>(json!({ "text": terminal.surface.tail(lines) }))
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    let list_core = Arc::clone(core);
    r.command("terminal.list", "terminal.command.list", move |_, args| {
        let bench = args.str("bench_path").map(str::to_owned);
        let terminals = list_core
            .with_hub(|hub| hub.list(bench.as_deref()))
            .map_err(command_error)?;
        Ok(json!({ "terminals": terminals }))
    });

    let copy_core = Arc::clone(core);
    r.command("terminal.copy", "terminal.command.copy", move |cx, _| {
        let settings = TermSettings::from_ctx(cx);
        copy_core
            .with_hub(|hub| {
                let terminal = hub
                    .focused_terminal()
                    .ok_or_else(|| "terminal not found".to_owned())?;
                let text = terminal.copy_text(&settings);
                let context = hub.ui_context.clone();
                if let Some(context) = context {
                    context.copy_text(text);
                }
                Ok::<_, String>(json!({ "copied": true }))
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    let paste_core = Arc::clone(core);
    r.command("terminal.paste", "terminal.command.paste", move |_, _| {
        paste_core
            .with_hub(|hub| {
                let terminal = hub
                    .focused_terminal_mut()
                    .ok_or_else(|| "terminal not found".to_owned())?;
                terminal.paste_request = true;
                if let Some(context) = hub.ui_context.clone() {
                    context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                Ok::<_, String>(json!({}))
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    let search_core = Arc::clone(core);
    r.command("terminal.search", "terminal.command.search", move |_, _| {
        search_core
            .with_hub(|hub| {
                hub.focused_terminal_mut()
                    .ok_or_else(|| "terminal not found".to_owned())
                    .map(|terminal| {
                        terminal.toggle_search();
                        json!({})
                    })
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    let clear_core = Arc::clone(core);
    r.command("terminal.clear", "terminal.command.clear", move |_, _| {
        clear_core
            .with_hub(|hub| {
                hub.focused_terminal_mut()
                    .ok_or_else(|| "terminal not found".to_owned())
                    .map(|terminal| {
                        terminal.clear();
                        json!({})
                    })
            })
            .map_err(command_error)?
            .map_err(command_error)
    });

    for (id, title, direction) in [
        (
            "terminal.split.right",
            "terminal.command.split-right",
            SplitDir::Columns,
        ),
        (
            "terminal.split.down",
            "terminal.command.split-down",
            SplitDir::Rows,
        ),
    ] {
        let split_core = Arc::clone(core);
        r.command(id, title, move |cx, _| {
            let settings = TermSettings::from_ctx(cx);
            let panes = split_core.with_hub(|hub| hub.pane_count()).unwrap_or(0);
            if panes >= MAX_PANES {
                let max = MAX_PANES.to_string();
                return Err(KernelError::Message(
                    cx.t_args("terminal.view.max-panes", &[("max", &max)]),
                ));
            }
            let id = split_core
                .with_hub(|hub| hub.split(direction, &settings))
                .map_err(command_error)?
                .map_err(command_error)?;
            Ok(json!({ "terminal_id": id.to_string() }))
        });
    }

    let next_core = Arc::clone(core);
    r.command(
        "terminal.pane.next",
        "terminal.command.pane-next",
        move |_, _| {
            next_core
                .with_hub(|hub| {
                    hub.focus_next();
                    json!({})
                })
                .map_err(command_error)
        },
    );
    let previous_core = Arc::clone(core);
    r.command(
        "terminal.pane.previous",
        "terminal.command.pane-previous",
        move |_, _| {
            previous_core
                .with_hub(|hub| {
                    hub.focus_previous();
                    json!({})
                })
                .map_err(command_error)
        },
    );
    let close_pane_core = Arc::clone(core);
    r.command(
        "terminal.pane.close",
        "terminal.command.pane-close",
        move |cx, _| {
            let settings = TermSettings::from_ctx(cx);
            let needs = close_pane_core
                .with_hub(|hub| hub.request_close(CloseTarget::Pane(hub.focused), &settings))
                .map_err(command_error)?;
            Ok(json!({ "needs_confirmation": needs }))
        },
    );
    let close_tab_core = Arc::clone(core);
    r.command(
        "terminal.tab.close",
        "terminal.command.tab-close",
        move |cx, _| {
            let settings = TermSettings::from_ctx(cx);
            let needs = close_tab_core
                .with_hub(|hub| {
                    hub.active()
                        .map(|(pane, terminal)| {
                            hub.request_close(CloseTarget::Tab(pane, terminal), &settings)
                        })
                        .unwrap_or(false)
                })
                .map_err(command_error)?;
            Ok(json!({ "needs_confirmation": needs }))
        },
    );
    let new_tab_core = Arc::clone(core);
    r.command(
        "terminal.new_tab",
        "terminal.command.new-tab",
        move |cx, _| {
            let settings = TermSettings::from_ctx(cx);
            let id = new_tab_core
                .with_hub(|hub| hub.new_tab(&settings, None))
                .map_err(command_error)?
                .map_err(command_error)?;
            Ok(json!({ "terminal_id": id.to_string() }))
        },
    );
    let focus_core = Arc::clone(core);
    r.command("terminal.focus", "terminal.command.focus", move |_, args| {
        let id = args
            .required_str("terminal_id")?
            .parse::<u64>()
            .map_err(|_| KernelError::Message("invalid terminal_id".to_owned()))?;
        let focused = focus_core
            .with_hub(|hub| hub.focus_terminal(id))
            .map_err(command_error)?;
        Ok(json!({ "focused": focused }))
    });

    let toggle_core = Arc::clone(core);
    r.command("terminal.toggle", "terminal.command.toggle", move |cx, _| {
        let settings = TermSettings::from_ctx(cx);
        let id = toggle_core
            .with_hub(|hub| {
                if hub.root.is_none() {
                    hub.open(&settings, OpenRequest::default())
                } else {
                    hub.active()
                        .map(|(_, terminal)| terminal)
                        .ok_or_else(|| "terminal not found".to_owned())
                }
            })
            .map_err(command_error)?
            .map_err(command_error)?;
        Ok(json!({ "terminal_id": id.to_string() }))
    });

    for (id, title, down) in [
        (
            "terminal.scroll.page_up",
            "terminal.command.scroll-page-up",
            false,
        ),
        (
            "terminal.scroll.page_down",
            "terminal.command.scroll-page-down",
            true,
        ),
    ] {
        let scroll_core = Arc::clone(core);
        r.command(id, title, move |_, _| {
            scroll_core
                .with_hub(|hub| {
                    hub.focused_terminal_mut()
                        .ok_or_else(|| "terminal not found".to_owned())
                        .map(|terminal| {
                            terminal.scroll_page(down);
                            json!({})
                        })
                })
                .map_err(command_error)?
                .map_err(command_error)
        });
    }
    for (id, title, bottom) in [
        ("terminal.scroll.top", "terminal.command.scroll-top", false),
        (
            "terminal.scroll.bottom",
            "terminal.command.scroll-bottom",
            true,
        ),
    ] {
        let scroll_core = Arc::clone(core);
        r.command(id, title, move |_, _| {
            scroll_core
                .with_hub(|hub| {
                    hub.focused_terminal_mut()
                        .ok_or_else(|| "terminal not found".to_owned())
                        .map(|terminal| {
                            if bottom {
                                terminal.scroll_bottom();
                            } else {
                                terminal.scroll_top();
                            }
                            json!({})
                        })
                })
                .map_err(command_error)?
                .map_err(command_error)
        });
    }
}

pub(super) fn recipe_bench(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("bench.current", Args::new()).ok()
        .and_then(|value| value.get("path").and_then(serde_json::Value::as_str).map(PathBuf::from))
}


fn command_error(message: impl ToString) -> KernelError {
    KernelError::Message(message.to_string())
}


