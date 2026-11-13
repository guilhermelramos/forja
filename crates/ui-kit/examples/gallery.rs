//! Galeria Brasa. `cargo run -p forja-ui-kit --example gallery -- --theme dark --screenshot saida.png`

use std::path::PathBuf;

use eframe::egui;
use forja_ui_kit::{
    button, card, context_menu, install_fonts, list, menu_panel, modal_preview, progress_bar,
    segmented, select, stage_badge, tabs, text_field, toast, toggle, tree, virtual_table,
    ButtonKind, Column, ListItem, MenuItem, Stage, ToastKind, Tokens, TreeNode,
};

fn main() -> eframe::Result {
    let args = Args::parse();
    let tokens = match args.theme.as_str() {
        "light" | "claro" => Tokens::brasa_light(),
        _ => Tokens::brasa_dark(),
    };
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 1180.0])
        .with_title("Forja — Galeria Brasa");
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let shot = args.screenshot.clone();
    eframe::run_native(
        "forja-gallery",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx).expect("Segoe UI e Cascadia Code em C:\\Windows\\Fonts");
            forja_ui_kit::apply(&cc.egui_ctx, &tokens);
            Ok(Box::new(Galeria::new(tokens, shot)))
        }),
    )
}

struct Args {
    theme: String,
    screenshot: Option<PathBuf>,
}

impl Args {
    fn parse() -> Self {
        let mut theme = "dark".to_owned();
        let mut screenshot = None;
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--theme" => {
                    theme = args.get(index + 1).cloned().unwrap_or(theme);
                    index += 2;
                }
                "--screenshot" => {
                    if let Some(path) = args.get(index + 1) {
                        screenshot = Some(PathBuf::from(path));
                    }
                    index += 2;
                }
                _ => index += 1,
            }
        }
        Self { theme, screenshot }
    }
}

struct Galeria {
    tokens: Tokens,
    shot: Option<PathBuf>,
    frame: u32,
    saved: bool,
    name: String,
    stage: usize,
    forged: bool,
    quiet: bool,
    segment: usize,
    tab: usize,
    list: Option<usize>,
    modal: bool,
    tree: Vec<TreeNode>,
}

impl Galeria {
    fn new(tokens: Tokens, shot: Option<PathBuf>) -> Self {
        Self {
            tokens,
            shot,
            frame: 0,
            saved: false,
            name: String::new(),
            stage: 1,
            forged: true,
            quiet: false,
            segment: 0,
            tab: 0,
            list: Some(0),
            modal: false,
            tree: vec![TreeNode::branch(
                "Projeto",
                vec![
                    TreeNode {
                        label: "Bancadas".to_owned(),
                        open: false,
                        children: vec![TreeNode::leaf("principal"), TreeNode::leaf("pedido-12")],
                    },
                    TreeNode::leaf("Ajudantes"),
                ],
            )],
        }
    }
}

impl eframe::App for Galeria {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        forja_ui_kit::apply(ctx, &self.tokens);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                self.left(&mut columns[0]);
                self.right(&mut columns[1]);
            });
        });
        self.capture(ctx);
    }
}

impl Galeria {
    fn left(&mut self, ui: &mut egui::Ui) {
        ui.heading("Galeria Brasa");
        ui.label(self.tokens.name.as_str());
        ui.horizontal(|ui| {
            let _ = button(ui, ButtonKind::Primary, "Forjar");
            let _ = button(ui, ButtonKind::Secondary, "Guardar");
            let _ = button(ui, ButtonKind::Ghost, "Cancelar");
            let _ = button(ui, ButtonKind::Danger, "Descartar");
            ui.add_enabled_ui(false, |ui| {
                let _ = button(ui, ButtonKind::Primary, "Indisponível");
            });
        });
        ui.label("Nome da bancada");
        let field = text_field(ui, &mut self.name, "peça em curso");
        if self.shot.is_some() && self.frame == 2 {
            field.request_focus();
        }
        ui.label("Etapa");
        select(
            ui,
            &mut self.stage,
            &["Rascunho", "Forjando", "Revisão", "Pronto"],
        );
        ui.horizontal(|ui| {
            toggle(ui, &mut self.forged, "Ajudante ativo");
            toggle(ui, &mut self.quiet, "Menos movimento");
        });
        if self.quiet != self.tokens.reduce_motion {
            self.tokens.reduce_motion = self.quiet;
        }
        segmented(
            ui,
            &mut self.segment,
            &["Terminal", "Ajudante", "Diferenças"],
        );
        tabs(ui, &mut self.tab, &["Quadro", "Pedidos", "Acervo"]);
        ui.horizontal(|ui| {
            stage_badge(ui, Stage::Draft, "Rascunho");
            stage_badge(ui, Stage::Forging, "Forjando");
            stage_badge(ui, Stage::Review, "Revisão");
            stage_badge(ui, Stage::Ready, "Pronto");
        });
        ui.label("Progresso");
        progress_bar(ui, 0.15);
        progress_bar(ui, 0.62);
        progress_bar(ui, 1.0);
        ui.add_space(4.0);
        toast(ui, ToastKind::Info, "Pedido aberto no balcão.");
        toast(ui, ToastKind::Ok, "Entrega registrada.");
        toast(ui, ToastKind::Spark, "A bancada espera você.");
        toast(ui, ToastKind::Danger, "O preparo falhou. Rode de novo.");
    }

    fn right(&mut self, ui: &mut egui::Ui) {
        card(ui, Some("Bancadas"), |ui| {
            list(
                ui,
                &[
                    ListItem {
                        label: "principal",
                        detail: Some("pronta"),
                    },
                    ListItem {
                        label: "pedido-12",
                        detail: Some("forjando"),
                    },
                    ListItem {
                        label: "revisão-7",
                        detail: Some("revisão"),
                    },
                ],
                &mut self.list,
            );
        });
        ui.label("Arquivos");
        tree(ui, &mut self.tree);
        ui.label("Pedidos");
        let rows = rows();
        virtual_table(
            ui,
            &[
                Column {
                    title: "Peça",
                    width: 180.0,
                },
                Column {
                    title: "Etapa",
                    width: 120.0,
                },
                Column {
                    title: "Ajudante",
                    width: 140.0,
                },
            ],
            rows.len(),
            64.0,
            |ui, row, column| {
                let text = rows[row][column];
                ui.label(text);
            },
        );
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(260.0);
                ui.set_max_width(280.0);
                ui.label("Menu");
                let _ = context_menu(
                    ui,
                    "Ações",
                    &[
                        MenuItem {
                            label: "Guardar",
                            danger: false,
                        },
                        MenuItem {
                            label: "Renomear",
                            danger: false,
                        },
                        MenuItem {
                            label: "Descartar",
                            danger: true,
                        },
                    ],
                );
                menu_panel(
                    ui,
                    &[
                        MenuItem {
                            label: "Guardar",
                            danger: false,
                        },
                        MenuItem {
                            label: "Renomear",
                            danger: false,
                        },
                        MenuItem {
                            label: "Descartar",
                            danger: true,
                        },
                    ],
                );
            });
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                ui.set_max_width(320.0);
                ui.label("Modal");
                if button(ui, ButtonKind::Secondary, "Abrir modal").clicked() {
                    self.modal = true;
                }
                modal_preview(ui, "Guardar bancada", |ui| {
                    ui.label("A branch fica no projeto.");
                    ui.horizontal(|ui| {
                        let _ = button(ui, ButtonKind::Primary, "Guardar");
                        let _ = button(ui, ButtonKind::Ghost, "Cancelar");
                    });
                });
            });
        });
        if self.modal {
            forja_ui_kit::modal(ui.ctx(), &mut self.modal, "Guardar bancada", |ui| {
                ui.label("A branch fica no projeto.");
            });
        }
    }

    fn capture(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.shot else {
            return;
        };
        if self.saved {
            return;
        }
        ctx.request_repaint();
        if self.frame >= 8 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            if let Err(err) = save_png(path, &image) {
                eprintln!("galeria: {err}");
                std::process::exit(1);
            }
            self.saved = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frame > 240 {
            eprintln!("galeria: o screenshot não chegou");
            std::process::exit(1);
        }
    }
}

fn rows() -> [[&'static str; 3]; 12] {
    [
        ["pedido-12", "Forjando", "ajudante"],
        ["pedido-13", "Revisão", "ajudante"],
        ["pedido-14", "Rascunho", "—"],
        ["pedido-15", "Pronto", "ajudante"],
        ["pedido-16", "Forjando", "ajudante"],
        ["pedido-17", "Revisão", "ajudante"],
        ["pedido-18", "Pronto", "ajudante"],
        ["pedido-19", "Rascunho", "—"],
        ["pedido-20", "Forjando", "ajudante"],
        ["pedido-21", "Revisão", "ajudante"],
        ["pedido-22", "Pronto", "ajudante"],
        ["pedido-23", "Forjando", "ajudante"],
    ]
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }
    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&pixel.to_array());
    }
    let width = u32::try_from(image.width()).map_err(|_| "largura inválida".to_owned())?;
    let height = u32::try_from(image.height()).map_err(|_| "altura inválida".to_owned())?;
    image::save_buffer(path, &bytes, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|err| err.to_string())
}
