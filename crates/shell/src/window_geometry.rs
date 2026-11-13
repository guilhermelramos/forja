//! Window geometry persistence, isolated from the shell's other stored state.
use eframe::egui;
use forja_kernel::Runtime;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry { pub x: f32, pub y: f32, pub width: f32, pub height: f32 }

impl Geometry {
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        let number = |key: &str| value.get(key)?.as_f64().map(|v| v as f32);
        let geometry = Self { x: number("x")?, y: number("y")?, width: number("width")?, height: number("height")? };
        geometry.valid().then_some(geometry)
    }
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height].iter().all(|v| v.is_finite())
            && (900.0..=16384.0).contains(&self.width)
            && (600.0..=16384.0).contains(&self.height)
            && self.x.abs() <= 65536.0 && self.y.abs() <= 65536.0
    }

    pub fn from_viewport(viewport: &egui::ViewportInfo) -> Option<Self> {
        if viewport.maximized.unwrap_or(false) || viewport.minimized.unwrap_or(false) || viewport.fullscreen.unwrap_or(false) { return None; }
        let outer = viewport.outer_rect?;
        let inner = viewport.inner_rect?;
        let result = Self { x: outer.min.x, y: outer.min.y, width: inner.width(), height: inner.height() };
        result.valid().then_some(result)
    }
}

#[derive(Default)]
struct WriterState { pending: Option<Geometry>, busy: bool }

#[derive(Default)]
pub struct GeometryWriter { state: Arc<Mutex<WriterState>>, last: Option<Geometry> }
impl GeometryWriter {
    pub fn busy(&self) -> bool {
        self.state.lock().map(|state| state.busy).unwrap_or(false)
    }
    pub fn record(&mut self, runtime: &mut Runtime, geometry: Geometry) {
        if self.last == Some(geometry) { return; }
        self.last = Some(geometry);
        let Ok(mut state) = self.state.lock() else { return; };
        state.pending = Some(geometry);
        if state.busy { return; }
        state.busy = true;
        drop(state);
        let state = Arc::clone(&self.state);
        let path = runtime.store().path_for("window_geometry");
        runtime.ctx("shell").spawn(move || loop {
            let next = match state.lock() {
                Ok(mut state) => match state.pending.take() {
                    Some(next) => next,
                    None => { state.busy = false; break; }
                },
                Err(_) => break,
            };
            if let Ok(bytes) = serde_json::to_vec(&serde_json::json!({
                "x": next.x, "y": next.y, "width": next.width, "height": next.height,
            })) {
                let _ = forja_kernel::store::write_atomic_bytes(&path, &bytes);
            }
        }, |_, _| {});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_geometry() {
        assert!(Geometry { x: -100.0, y: 0.0, width: 1280.0, height: 800.0 }.valid());
        assert!(!Geometry { x: f32::NAN, y: 0.0, width: 1280.0, height: 800.0 }.valid());
        assert!(!Geometry { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }.valid());
    }

    #[test]
    fn writer_persists_the_most_recent_geometry() {
        let dir = std::env::temp_dir().join(format!("forja-geometry-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut runtime = crate::app::build_runtime(None, Some(dir.clone()), Some(dir.join("user")), false).unwrap();
        let mut writer = GeometryWriter::default();
        writer.record(&mut runtime, Geometry { x: 1.0, y: 2.0, width: 1280.0, height: 800.0 });
        let last = Geometry { x: 10.0, y: 20.0, width: 1400.0, height: 900.0 };
        writer.record(&mut runtime, last);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while writer.busy() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!writer.busy());
        let stored = runtime.store().load::<serde_json::Value>("window_geometry").into_option().unwrap();
        assert_eq!(Geometry::from_json(&stored), Some(last));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
