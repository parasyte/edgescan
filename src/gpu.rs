//! Platform-neutral GPU state management and rendering.

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use thiserror::Error;
use winit::dpi::PhysicalSize;

#[derive(Debug, Error)]
pub enum Error {
    /// No suitable [`wgpu::Adapter`] found
    #[error("No suitable `wgpu::Adapter` found.")]
    AdapterNotFound,
    /// Equivalent to [`wgpu::RequestDeviceError`]
    #[error("No wgpu::Device found.")]
    DeviceNotFound(#[from] wgpu::RequestDeviceError),
    /// Equivalent to [`wgpu::SurfaceError`]
    #[error("The GPU failed to acquire a surface frame.")]
    Surface(#[from] wgpu::SurfaceError),
    /// Equivalent to [`wgpu::CreateSurfaceError`]
    #[error("Unable to create a surface.")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
}

pub struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) texture_format: wgpu::TextureFormat,
    surface: wgpu::Surface,
    window_size: winit::dpi::PhysicalSize<u32>,
    alpha_mode: wgpu::CompositeAlphaMode,
}

impl Gpu {
    /// Create a new GPU manager.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the window reference outlives the returned `Gpu` instance.
    pub unsafe fn new<W: HasRawDisplayHandle + HasRawWindowHandle>(
        window: &W,
        window_size: PhysicalSize<u32>,
    ) -> Result<Self, Error> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let surface = instance.create_surface(window)?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            power_preference: wgpu::PowerPreference::HighPerformance,
        });
        let adapter = pollster::block_on(adapter).ok_or(Error::AdapterNotFound)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))?;

        let texture_format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let surface_capabilities = surface.get_capabilities(&adapter);
        let alpha_mode = surface_capabilities.alpha_modes[0];

        let gpu = Self {
            device,
            queue,
            texture_format,
            surface,
            window_size,
            alpha_mode,
        };
        gpu.reconfigure_surface();

        Ok(gpu)
    }

    fn reconfigure_surface(&self) {
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.texture_format,
                width: self.window_size.width,
                height: self.window_size.height,
                present_mode: wgpu::PresentMode::AutoNoVsync,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
            },
        )
    }

    pub(crate) fn resize(&mut self, window_size: PhysicalSize<u32>) {
        self.window_size = window_size;
        self.reconfigure_surface();
    }

    pub(crate) fn prepare(
        &mut self,
    ) -> Result<(wgpu::CommandEncoder, wgpu::SurfaceTexture), Error> {
        let frame = self
            .surface
            .get_current_texture()
            .or_else(|err| match err {
                wgpu::SurfaceError::Outdated => {
                    // Recreate the swap chain to mitigate race condition on drawing surface resize.
                    self.reconfigure_surface();
                    self.surface.get_current_texture()
                }
                err => Err(err),
            })?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_command_encoder"),
            });

        Ok((encoder, frame))
    }
}
