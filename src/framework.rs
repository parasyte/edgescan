use crate::{config::Config, gui::Gui};
use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::{winit::Painter, WgpuConfiguration};
use egui_winit::EventResponse;
use std::time::Duration;
use winit::{event_loop::EventLoopWindowTarget, window::Window};

/// Manages all state required for rendering egui.
pub struct Framework {
    // State for egui.
    egui_ctx: Context,
    egui_state: egui_winit::State,
    painter: Painter,
    clipped_primitives: Vec<ClippedPrimitive>,
    textures_delta: TexturesDelta,

    // Configuration for the app.
    config: Config,

    // State for the GUI.
    gui: Gui,
}

impl Framework {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        scale_factor: f64,
        config: Config,
    ) -> Self {
        let egui_ctx = Context::default();
        let mut egui_state = egui_winit::State::new(event_loop);
        let painter = Painter::new(WgpuConfiguration::default(), 1, 0);
        let gui = Gui::new();

        if let Some(max_texture_size) = painter.max_texture_side() {
            egui_state.set_max_texture_side(max_texture_size);
        }
        egui_state.set_pixels_per_point(scale_factor as f32);

        Self {
            egui_ctx,
            egui_state,
            painter,
            clipped_primitives: vec![],
            textures_delta: TexturesDelta::default(),
            config,
            gui,
        }
    }

    pub unsafe fn set_window(&mut self, window: &Window) {
        self.painter.set_window(Some(window));
    }

    pub fn config(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Handle input events from the window manager.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> EventResponse {
        self.egui_state.on_event(&self.egui_ctx, event)
    }

    /// Resize egui.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.set_window_size(width, height);
            self.painter.on_window_resized(width, height);
        }
    }

    /// Prepare egui.
    pub fn prepare(&mut self, window: &Window) -> Duration {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw the demo application.
            self.gui.ui(egui_ctx, window);
        });

        self.egui_state
            .handle_platform_output(window, &self.egui_ctx, output.platform_output);

        self.clipped_primitives = self.egui_ctx.tessellate(output.shapes);
        self.textures_delta = output.textures_delta;

        output.repaint_after
    }

    pub fn render(&mut self) {
        let pixels_per_point = self.egui_state.pixels_per_point();

        self.painter.paint_and_update_textures(
            pixels_per_point,
            egui::Rgba::BLACK,
            &self.clipped_primitives,
            &self.textures_delta,
        );
    }
}
