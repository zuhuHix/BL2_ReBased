//! Owner: lane H1. The wgpu side of the host: instance, adapter, device, surface, and one frame that clears and
//! presents.
//!
//! AI-assisted (Claude Code). Written from the wgpu 30.0.1 API, not from any listing.
//!
//! No device or surface error panics. wgpu reports most errors through the device's uncaptured-error handler (whose
//! default panics) and the device-lost callback; both are replaced here by handlers that record the first fault, and
//! every frame checks it and returns it as a [`GpuError`]. The one panic left in reach is `wgpu::Instance::new` with
//! no backend compiled in, which the default features rule out on Windows, Linux and macOS.

use std::fmt;
use std::sync::{Arc, Mutex};

use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

use crate::executor::block_on;

/// What the probe and the window run print about the adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AdapterSummary {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
}

impl AdapterSummary {
    pub fn from_info(info: &wgpu::AdapterInfo) -> Self {
        let driver = match (info.driver.is_empty(), info.driver_info.is_empty()) {
            (false, false) => format!("{} {}", info.driver, info.driver_info),
            (false, true) => info.driver.clone(),
            (true, _) => info.driver_info.clone(),
        };
        AdapterSummary {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver,
        }
    }
}

/// Why the GPU side stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GpuError {
    /// No adapter matched (no GPU, no driver, or none that can present to the window).
    NoAdapter(String),
    /// The surface could not be created or configured for this window.
    Surface(String),
    /// The adapter refused the device.
    Device(String),
    /// The device was lost after it was created.
    DeviceLost(String),
    /// wgpu reported a validation or out-of-memory error.
    Validation(String),
    /// The renderer plugged in through [`Gpu::render_with`] could not encode its frame.
    Draw(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapter(m) => write!(f, "no GPU adapter: {m}"),
            GpuError::Surface(m) => write!(f, "window surface: {m}"),
            GpuError::Device(m) => write!(f, "device request refused: {m}"),
            GpuError::DeviceLost(m) => write!(f, "device lost: {m}"),
            GpuError::Validation(m) => write!(f, "wgpu error: {m}"),
            GpuError::Draw(m) => write!(f, "drawing the frame: {m}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Creates an instance and an adapter with no window, for CI and other machines.
pub fn headless_probe() -> Result<AdapterSummary, GpuError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
    Ok(AdapterSummary::from_info(&adapter.get_info()))
}

/// What one call to [`Gpu::render`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameOutcome {
    Presented,
    /// No frame this time (minimised or occluded window, a timeout, or a surface that had to be reconfigured).
    Skipped,
}

/// The first fault wgpu reported through a callback.
#[derive(Default)]
struct Faults(Mutex<Option<GpuError>>);

impl Faults {
    fn record(&self, error: GpuError) {
        let mut slot = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn take(&self) -> Option<GpuError> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take()
    }
}

/// What [`Gpu::render_with`] calls to encode a frame: the device, the queue, the encoder, the frame's view and its
/// size in pixels.
pub type DrawFrame<'a> = dyn FnMut(
        &wgpu::Device,
        &wgpu::Queue,
        &mut wgpu::CommandEncoder,
        &wgpu::TextureView,
        (u32, u32),
    ) -> Result<(), String>
    + 'a;

/// The device and the window's surface.
pub struct Gpu {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// False while the window has a zero-sized drawable area (minimised); nothing is acquired then.
    drawable: bool,
    faults: Arc<Faults>,
    pub adapter: AdapterSummary,
}

impl Gpu {
    pub fn new(window: Arc<Window>, display: OwnedDisplayHandle, vsync: bool) -> Result<Gpu, GpuError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display)));
        let surface = instance.create_surface(Arc::clone(&window)).map_err(|e| GpuError::Surface(e.to_string()))?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        let summary = AdapterSummary::from_info(&adapter.get_info());
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ow-host"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;

        let faults = Arc::new(Faults::default());
        let on_error = Arc::clone(&faults);
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| on_error.record(GpuError::Validation(e.to_string()))));
        let on_lost = Arc::clone(&faults);
        device.set_device_lost_callback(move |reason, message| {
            on_lost.record(GpuError::DeviceLost(format!("{reason:?}: {message}")));
        });

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| GpuError::Surface("the adapter cannot present to this window".into()))?;
        config.present_mode = if vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        // An sRGB format when the adapter offers one (lane H2's renderer writes linear colour).
        if let Some(&srgb) = surface.get_capabilities(&adapter).formats.iter().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        surface.configure(&device, &config);
        let gpu = Gpu {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            drawable: size.width > 0 && size.height > 0,
            faults,
            adapter: summary,
        };
        gpu.check()?;
        Ok(gpu)
    }

    /// The present mode asked for (`AutoVsync` or `AutoNoVsync`); wgpu picks the concrete mode from it.
    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.config.present_mode
    }

    /// Reconfigures the surface for a new drawable size; a zero size pauses drawing until the next non-zero one.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.drawable = width > 0 && height > 0;
        if self.drawable {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// The device, for a renderer to upload with (lane H2's seam).
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue, for a renderer to upload with.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The surface's texture format, which a renderer's pipelines target.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Clears the next frame to `clear` (linear RGBA) and presents it.
    pub fn render(&mut self, clear: [f64; 4]) -> Result<FrameOutcome, GpuError> {
        let [r, g, b, a] = clear;
        self.render_with(&mut |_, _, encoder, view, _| {
            drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }));
            Ok(())
        })
    }

    /// Acquires the next frame, lets `draw` encode into it (the device, the queue, the encoder, the frame's view and
    /// its size in pixels) and presents it. An error of `draw` is [`GpuError::Draw`]; nothing is presented then.
    pub fn render_with(
        &mut self,
        draw: &mut DrawFrame<'_>,
    ) -> Result<FrameOutcome, GpuError> {
        self.check()?;
        if !self.drawable {
            return Ok(FrameOutcome::Skipped);
        }
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(FrameOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(FrameOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(Arc::clone(&self.window))
                    .map_err(|e| GpuError::Surface(e.to_string()))?;
                self.surface.configure(&self.device, &self.config);
                return Ok(FrameOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(self
                    .faults
                    .take()
                    .unwrap_or_else(|| GpuError::Validation("acquiring the next frame failed validation".into())));
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ow-host frame") });
        let size = (self.config.width, self.config.height);
        draw(&self.device, &self.queue, &mut encoder, &view, size).map_err(GpuError::Draw)?;
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        if suboptimal {
            self.surface.configure(&self.device, &self.config);
        }
        self.check()?;
        Ok(FrameOutcome::Presented)
    }

    fn check(&self) -> Result<(), GpuError> {
        match self.faults.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_fault_is_kept() {
        let faults = Faults::default();
        faults.record(GpuError::Validation("first".into()));
        faults.record(GpuError::DeviceLost("second".into()));
        assert_eq!(faults.take(), Some(GpuError::Validation("first".into())));
        assert_eq!(faults.take(), None);
    }

    #[test]
    fn errors_read_as_one_line() {
        assert_eq!(GpuError::NoAdapter("none found".into()).to_string(), "no GPU adapter: none found");
        assert_eq!(GpuError::DeviceLost("Unknown: reset".into()).to_string(), "device lost: Unknown: reset");
    }
}
