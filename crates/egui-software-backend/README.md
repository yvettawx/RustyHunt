> **Vendored copy.** This is egui_software_backend 0.0.3 by DGriffin91
> (https://github.com/DGriffin91/egui_software_backend, MIT OR Apache-2.0), copied into RustyHunt
> and brought up from egui 0.34 to 0.37. Changes from upstream:
>
> - `ahash` is a direct dependency (egui no longer re-exports it).
> - `TexturesDelta::set` holds a list of deltas per texture id; each one is applied.
> - `ViewportBuilder` gained `monitor`; `ViewportCommand` gained `SetMonitor` and
>   `RequestPaintWhileHidden` (ignored by the winit runner).
> - `copy_changed_tiles` copies only the tiles that changed this frame into a buffer kept between frames.
> - The canvas is also updated when primitives only disappeared (their tiles kept the old pixels).
> - The `test_render` module, the tests comparing against the GPU renderer and the examples are left out.
> - Two newer clippy lints are allowed in `Cargo.toml`; `rustfmt.toml` keeps the upstream formatting.

# CPU software render backend for [egui](https://github.com/emilk/egui)
![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg) [![Crates.io](https://img.shields.io/crates/v/egui_software_backend.svg)](https://crates.io/crates/egui_software_backend)
[![Docs](https://docs.rs/egui_software_backend/badge.svg)](https://docs.rs/egui_software_backend/latest/egui_software_backend/)

![demo](demo.png)

```rs
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
let buffer = &mut vec![[0u8; 4]; 512 * 512];
let mut buffer_ref = BufferMutRef::new(buffer, 512, 512);
let ctx = egui::Context::default();
let mut demo = egui_demo_lib::DemoWindows::default();
let mut sw_render = EguiSoftwareRender::new(ColorFieldOrder::Bgra);

let out = ctx.run(egui::RawInput::default(), |ctx| {
    demo.ui(ctx);
});

let primitives = ctx.tessellate(out.shapes, out.pixels_per_point);

sw_render.render(
    &mut buffer_ref,
    &primitives,
    &out.textures_delta,
    out.pixels_per_point,
);
```

## winit quickstart
```rust
use egui::vec2;
use egui_software_backend::{SoftwareBackend, SoftwareBackendAppConfiguration};

struct EguiApp {}

impl EguiApp {
    fn new(context: egui::Context) -> Self {
        egui_extras::install_image_loaders(&context);
        EguiApp {}
    }
}

impl egui_software_backend::App for EguiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _backend: &mut SoftwareBackend) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("Hello World!");
        });
    }
}

fn main() {
    let settings = SoftwareBackendAppConfiguration::new()
        .inner_size(Some(vec2(500.0, 300.0)))
        .title(Some("Simple example".to_string()));

    egui_software_backend::run_app_with_software_backend(settings, EguiApp::new)
        //Can fail if winit fails to create the window
        .expect("Failed to run app")
}
```

[egui_backend_selector](https://github.com/AlexanderSchuetz97/egui_backend_selector) can be used in conjunction with this crate to automatically fallback to using this software renderer at runtime.

## Other examples
- bevy + softbuffer see examples/bevy_example folder

## egui version mapping
| egui_software_backend | egui   |
|-----------------------|--------|
| 0.0.3                 | 0.34.X |
| 0.0.2                 | 0.33.X |
| 0.0.1                 | 0.33.X |

