use crate::gpu::{Error, Gpu};
use crate::{config::Config, gui::Gui};
use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use egui_winit::EventResponse;
use std::time::Duration;
use winit::{dpi::PhysicalSize, event_loop::EventLoopWindowTarget, window::Window};

/// Manages all state required for rendering egui.
pub struct Framework {
    // State for egui.
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    clipped_primitives: Vec<ClippedPrimitive>,
    textures_delta: TexturesDelta,
    gpu: Gpu,

    // Configuration for the app.
    config: Config,

    // State for the GUI.
    gui: Gui,
}

impl Framework {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        size: PhysicalSize<u32>,
        scale_factor: f64,
        config: Config,
        gpu: Gpu,
    ) -> Self {
        let width = size.width;
        let height = size.height;
        let scale_factor = scale_factor as f32;
        let max_texture_size = gpu.device.limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();
        let mut egui_state = egui_winit::State::new(event_loop);
        egui_state.set_max_texture_side(max_texture_size);
        egui_state.set_pixels_per_point(scale_factor);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = Renderer::new(&gpu.device, gpu.texture_format, None, 1);
        let gui = Gui::new();

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            clipped_primitives: vec![],
            textures_delta: TexturesDelta::default(),
            gpu,
            config,
            gui,
        }
    }

    pub fn config(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Handle input events from the window manager.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> EventResponse {
        self.egui_state.on_event(&self.egui_ctx, event)
    }

    /// Resize egui.
    pub fn resize(&mut self, window_size: PhysicalSize<u32>, scale_factor: f64) {
        let PhysicalSize { width, height } = window_size;
        if width > 0 && height > 0 {
            self.gpu.resize(window_size);
            self.config.set_window_size(width, height, scale_factor);
            self.screen_descriptor.size_in_pixels = [width, height];
            self.screen_descriptor.pixels_per_point = scale_factor as f32;
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

    pub fn render(&mut self) -> Result<(), Error> {
        let (mut encoder, frame) = self.gpu.prepare()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Upload all resources to the GPU.
        for (id, image_delta) in &self.textures_delta.set {
            self.renderer
                .update_texture(&self.gpu.device, &self.gpu.queue, *id, image_delta);
        }
        self.renderer.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &self.clipped_primitives,
            &self.screen_descriptor,
        );

        // Render egui with WGPU
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer.render(
                &mut rpass,
                &self.clipped_primitives,
                &self.screen_descriptor,
            );
        }

        // Cleanup
        let textures = std::mem::take(&mut self.textures_delta);
        for id in &textures.free {
            self.renderer.free_texture(id);
        }

        // Complete frame
        self.gpu.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(())
    }
}
