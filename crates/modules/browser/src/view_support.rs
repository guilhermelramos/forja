//! Small helpers shared by the browser view.

use forja_ui_kit::egui;

pub fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}

pub fn home_html(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>html,body{{margin:0;background:#121010;color:#EDE7E1;font:16px system-ui;}}\
         main{{max-width:720px;margin:0 auto;padding:56px 24px;}}\
         h1{{font-size:36px;margin:0 0 12px;color:#F2762E;}}p{{line-height:1.6;color:#A69C93;}}</style>\
         </head><body><main><h1>{}</h1><p>{}</p></main></body></html>",
        escape(title),
        escape(title),
        escape(body)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
