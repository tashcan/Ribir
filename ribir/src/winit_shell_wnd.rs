use ribir_core::{
  prelude::*,
  window::{ShellWindow, WindowId},
};
use ribir_gpu::WgpuTexture;
use winit::event_loop::EventLoopWindowTarget;

pub struct WinitShellWnd {
  pub(crate) winit_wnd: winit::window::Window,
  #[cfg(feature = "wgpu")]
  backend: WinitWgpu,
}

#[cfg(feature = "wgpu")]
struct WinitWgpu {
  surface: wgpu::Surface,
  backend: ribir_gpu::GPUBackend<ribir_gpu::WgpuImpl>,
  current_texture: Option<WgpuTexture>,
}

#[cfg(feature = "wgpu")]
impl WinitWgpu {
  fn new(window: &winit::window::Window) -> WinitWgpu {
    let instance = wgpu::Instance::new(<_>::default());
    let surface = unsafe { instance.create_surface(window).unwrap() };
    let wgpu = AppContext::wait_future(ribir_gpu::WgpuImpl::new(instance, Some(&surface)));
    let size = window.inner_size();
    surface.configure(
      wgpu.device(),
      &Self::surface_config(size.width, size.height),
    );

    WinitWgpu {
      surface,
      backend: ribir_gpu::GPUBackend::new(wgpu, AntiAliasing::Msaa4X),
      current_texture: None,
    }
  }

  pub fn on_resize(&mut self, size: DeviceSize) {
    self.surface.configure(
      self.backend.get_impl().device(),
      &Self::surface_config(size.width as u32, size.height as u32),
    );
  }

  fn surface_config(width: u32, height: u32) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      format: wgpu::TextureFormat::Bgra8Unorm,
      width,
      height,
      present_mode: wgpu::PresentMode::Fifo,
      alpha_mode: wgpu::CompositeAlphaMode::Auto,
      view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
    }
  }

  fn begin_frame(&mut self) {
    self.backend.begin_frame();
    assert!(self.current_texture.is_none());
    let surface_tex = self.surface.get_current_texture().unwrap();
    self.current_texture = Some(WgpuTexture::from_surface_tex(surface_tex));
  }

  fn draw_commands(
    &mut self,
    viewport: DeviceRect,
    commands: Vec<PaintCommand>,
    surface_color: Color,
  ) {
    let surface = self.current_texture.as_mut().unwrap();

    self
      .backend
      .draw_commands(viewport, commands, surface_color, surface);
  }

  fn end_frame(&mut self) {
    self.backend.end_frame();
    let surface = self
      .current_texture
      .take()
      .unwrap()
      .into_surface_texture()
      .unwrap();
    surface.present();
  }
}

impl ShellWindow for WinitShellWnd {
  fn id(&self) -> WindowId { new_id(self.winit_wnd.id()) }

  fn device_pixel_ratio(&self) -> f32 { self.winit_wnd.scale_factor() as f32 }

  fn inner_size(&self) -> Size {
    let size = self
      .winit_wnd
      .inner_size()
      .to_logical(self.winit_wnd.scale_factor());
    Size::new(size.width, size.height)
  }

  fn outer_size(&self) -> Size {
    let size = self
      .winit_wnd
      .outer_size()
      .to_logical(self.winit_wnd.scale_factor());
    Size::new(size.width, size.height)
  }

  fn set_size(&mut self, size: Size) {
    if self.inner_size() != size {
      self
        .winit_wnd
        .set_inner_size(winit::dpi::LogicalSize::new(size.width, size.height));
      self.on_resize(size)
    }
  }

  #[inline]
  fn set_cursor(&mut self, cursor: CursorIcon) { self.winit_wnd.set_cursor_icon(cursor) }

  #[inline]
  fn set_title(&mut self, title: &str) { self.winit_wnd.set_title(title) }

  #[inline]
  fn as_any(&self) -> &dyn std::any::Any { self }

  #[inline]
  fn as_any_mut(&mut self) -> &mut dyn Any { self }

  #[inline]
  fn begin_frame(&mut self) { self.backend.begin_frame() }

  #[inline]
  fn draw_commands(&mut self, viewport: Rect, mut commands: Vec<PaintCommand>, surface: Color) {
    commands.iter_mut().for_each(|c| match c {
      PaintCommand::ColorPath { path, .. }
      | PaintCommand::ImgPath { path, .. }
      | PaintCommand::Clip(path) => path.scale(self.winit_wnd.scale_factor() as f32),
      PaintCommand::PopClip => {}
    });

    let scale = self.winit_wnd.scale_factor() as f32;
    let viewport = viewport
      .scale(scale, scale)
      .round_out()
      .to_i32()
      .cast_unit();
    self.backend.draw_commands(viewport, commands, surface);
  }

  #[inline]
  fn end_frame(&mut self) { self.backend.end_frame() }
}

pub(crate) fn new_id(id: winit::window::WindowId) -> WindowId {
  let id: u64 = id.into();
  id.into()
}

impl WinitShellWnd {
  pub(crate) fn new<T>(size: Option<Size>, window_target: &EventLoopWindowTarget<T>) -> Self {
    let mut winit_wnd = winit::window::WindowBuilder::new();
    if let Some(size) = size {
      winit_wnd = winit_wnd.with_inner_size(winit::dpi::LogicalSize::new(size.width, size.height));
    }

    let winit_wnd = winit_wnd.build(window_target).unwrap();
    WinitShellWnd {
      #[cfg(feature = "wgpu")]
      backend: WinitWgpu::new(&winit_wnd),
      winit_wnd,
    }
  }

  pub fn on_resize(&mut self, size: Size) {
    let size = (size * self.device_pixel_ratio()).to_i32().cast_unit();
    self.backend.on_resize(size);
  }
}
