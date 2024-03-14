use crate::register_platform_app_events_handlers;
use crate::winit_shell_wnd::{new_id, WinitShellWnd};
use ribir_core::{prelude::*, timer::Timer, window::WindowId};
use std::convert::Infallible;
use winit::event::ElementState;
use winit::event_loop::EventLoopWindowTarget;

use winit::{
  event::{Event, Ime, KeyEvent, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub struct App {
  event_loop: EventLoop<AppEvent>,
  active_wnd: Option<WindowId>,
  events_stream: MutRefItemSubject<'static, AppEvent, Infallible>,
}

#[cfg(target_family = "wasm")]
pub struct App {
  event_loop_proxy: EventLoopProxy<AppEvent>,
  events_stream: MutRefItemSubject<'static, AppEvent, Infallible>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyEvent {
  pub key_code: Option<KeyCode>,
  pub modifiers: Option<ModifiersState>,
}

pub enum AppEvent {
  /// The event is sent when any future is waked to poll.
  FuturesWake,
  /// The event is sent when the application is be required to open a url. For
  /// example, it's launched from browser with a url.
  OpenUrl(String),
  /// The event is get global hotkey, it will receive the hotkey event.
  Hotkey(HotkeyEvent),
  /// The event is sent when the application window focus changed.
  WndFocusChanged(WindowId, bool),
  /// The custom event, you can send any data with this event.
  Custom(Box<dyn Any + Send>),
}

/// A sender to send event to the application event loop from which the
/// `EventSender` was created.
#[derive(Clone)]
pub struct EventSender(EventLoopProxy<AppEvent>);

#[derive(Clone)]
pub struct EventWaker(EventLoopProxy<AppEvent>);

impl App {
  pub fn events_stream() -> MutRefItemSubject<'static, AppEvent, Infallible> {
    App::shared().events_stream.clone()
  }

  #[track_caller]
  fn shared() -> &'static App { unsafe { Self::shared_mut() } }

  fn dispatch_wnd_native_event(wnd: &Window, event: WindowEvent) {
    static mut PRE_EDIT_HANDLE: PreEditHandle = PreEditHandle::new();
    match event {
      WindowEvent::KeyboardInput { event, .. } => {
        let KeyEvent {
          physical_key,
          logical_key,
          text,
          location,
          repeat,
          state,
          ..
        } = event;
        if unsafe { PRE_EDIT_HANDLE.is_in_pre_edit() } {
          return;
        }
        wnd.processes_keyboard_event(physical_key, logical_key, repeat, location, state);
        if state == ElementState::Pressed {
          if let Some(txt) = text {
            wnd.processes_receive_chars(txt.to_string());
          }
        }
      }
      WindowEvent::Ime(ime) => unsafe {
        PRE_EDIT_HANDLE.update(wnd, &ime);
      },
      WindowEvent::MouseInput { state, button, device_id, .. } => {
        if state == ElementState::Pressed {
          unsafe {
            PRE_EDIT_HANDLE.force_exit(wnd);
          }
        }
        wnd.process_mouse_input(device_id, state, button);
      }
      #[allow(deprecated)]
      _ => wnd.processes_native_event(event),
    }
  }

  fn event_loop_handle(event: Event<AppEvent>, loop_handle: &EventLoopWindowTarget<AppEvent>) {
    match event {
      Event::WindowEvent { event, window_id } => {
        let wnd_id = new_id(window_id);
        let Some(wnd) = AppCtx::get_window(wnd_id) else {
          return;
        };
        match event {
          WindowEvent::CloseRequested => {
            AppCtx::remove_wnd(wnd_id);
            if !AppCtx::has_wnd() {
              loop_handle.exit();
            }
          }
          WindowEvent::RedrawRequested => {
            if let Some(wnd) = AppCtx::get_window(wnd_id) {
              // if this frame is really draw, request another redraw. To make sure the draw
              // always end with a empty draw and emit an extra tick cycle message.
              if wnd.draw_frame() {
                request_redraw(&wnd);
              }
            }
          }
          WindowEvent::Resized(_) => {
            let size = wnd.shell_wnd().borrow().inner_size();
            wnd.shell_wnd().borrow_mut().on_resize(size);
            request_redraw(&wnd)
          }
          WindowEvent::Focused(focused) => {
            let mut event = AppEvent::WndFocusChanged(wnd_id, focused);
            let app = unsafe { App::shared_mut() };
            app.events_stream.next(&mut event);
          }
          event => {
            App::dispatch_wnd_native_event(&wnd, event);
          }
        }
        wnd.emit_events();
      }
      Event::AboutToWait => {
        let run_count = AppCtx::run_until_stalled();
        if run_count > 0 {
          for wnd in AppCtx::windows().borrow().values() {
            request_redraw(wnd);
          }
        }
        if run_count > 0 {
          loop_handle.set_control_flow(ControlFlow::Poll);
        } else if let Some(t) = Timer::recently_timeout() {
          let control = ControlFlow::wait_duration(t.duration_since(Instant::now()));
          loop_handle.set_control_flow(control);
        } else {
          loop_handle.set_control_flow(ControlFlow::Wait);
        };
      }
      Event::NewEvents(cause) => match cause {
        StartCause::Poll | StartCause::ResumeTimeReached { start: _, requested_resume: _ } => {
          Timer::wake_timeout_futures();
        }
        _ => (),
      },
      Event::UserEvent(mut event) => {
        AppCtx::spawn_local(async move {
          let app = unsafe { App::shared_mut() };
          app.events_stream.next(&mut event);
        })
        .unwrap();
      }
      _ => (),
    }
  }
}

#[cfg(target_family = "wasm")]
static mut APP: Option<App> = None;

#[cfg(target_family = "wasm")]
impl App {
  /// build the ribir application from the root widget.
  /// the ribir application will be running at the first canvas with class name
  /// of ribir_canvas
  pub async fn exec(root: impl WidgetBuilder + 'static) {
    const RIBIR_CANVAS: &str = "ribir_canvas";
    const RIBIR_CANVAS_USED: &str = "ribir_canvas_used";

    use web_sys::wasm_bindgen::JsCast;
    use winit::platform::web::EventLoopExtWebSys;
    let document = web_sys::window().unwrap().document().unwrap();
    let elems = document.get_elements_by_class_name(RIBIR_CANVAS);

    let len = elems.length();
    for idx in 0..len {
      if let Some(elem) = elems.get_with_index(idx) {
        let mut classes_name = elem.class_name();
        if !classes_name.split(" ").any(|v| v == RIBIR_CANVAS_USED) {
          let mut canvas = elem.clone().dyn_into::<web_sys::HtmlCanvasElement>();
          if canvas.is_err() {
            let child = document.create_element("canvas").unwrap();
            if elem.append_child(&child).is_ok() {
              canvas = child.dyn_into::<web_sys::HtmlCanvasElement>();
            }
          }
          if let Ok(canvas) = canvas {
            classes_name.push_str(&format!(" {}", RIBIR_CANVAS_USED));
            elem.set_class_name(&classes_name);

            let event_loop = EventLoopBuilder::with_user_event().build().unwrap();
            App::init(&event_loop);
            let shell_wnd = WinitShellWnd::new_with_canvas(canvas, &event_loop).await;
            AppCtx::new_window(Box::new(shell_wnd), root);
            event_loop.spawn(App::event_loop_handle);
            return;
          }
        }
      }
    }
  }

  /// Get a event sender of the application event loop, you can use this to send
  /// event.
  pub fn event_sender() -> EventSender {
    let proxy = App::shared().event_loop_proxy.clone();
    EventSender(proxy)
  }

  fn init(event_loop: &EventLoop<AppEvent>) {
    let waker = EventWaker(event_loop.create_proxy());
    unsafe {
      AppCtx::set_runtime_waker(Box::new(waker));
    }
    register_platform_app_events_handlers();
    unsafe {
      APP = Some(App {
        event_loop_proxy: event_loop.create_proxy(),
        events_stream: <_>::default(),
      });
    }
  }

  #[track_caller]
  unsafe fn shared_mut() -> &'static mut App { APP.as_mut().unwrap() }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl App {
  /// Start an application with the `root` widget, this will use the default
  /// theme to create an application and use the `root` widget to create a
  /// window, then run the application.
  #[track_caller]
  pub fn run(root: impl WidgetBuilder + 'static) {
    Self::new_window(root, None);
    App::exec()
  }

  /// Get a event sender of the application event loop, you can use this to send
  /// event.
  pub fn event_sender() -> EventSender {
    let proxy = App::shared().event_loop.create_proxy();
    EventSender(proxy)
  }

  /// create a new window with the `root` widget
  #[track_caller]
  pub fn new_window(root: impl WidgetBuilder + 'static, size: Option<Size>) -> std::rc::Rc<Window> {
    let app = unsafe { App::shared_mut() };
    let shell_wnd = WinitShellWnd::new(size, &app.event_loop);
    let wnd = AppCtx::new_window(Box::new(shell_wnd), root);

    if app.active_wnd.is_none() {
      app.active_wnd = Some(wnd.id());
    }
    wnd
  }

  pub fn active_window() -> std::rc::Rc<Window> {
    App::shared()
      .active_wnd
      .and_then(AppCtx::get_window)
      .expect("application at least have one window before use.")
  }

  /// set the window with `id` to be the active window, and the active window.
  #[track_caller]
  pub fn set_active_window(id: WindowId) {
    let app = unsafe { App::shared_mut() };
    app.active_wnd = Some(id);
    // todo: set the window to be the top window, but we not really support
    // multi window fully, implement this later.
    if let Some(wnd) = AppCtx::get_window(id) {
      let mut shell = wnd.shell_wnd().borrow_mut();
      if shell.is_minimized() {
        shell.set_minimized(false);
      }
      shell.focus_window();
    };
  }

  /// run the application, this will start the event loop and block the current
  /// thread until the application exit.
  #[track_caller]
  pub fn exec() {
    use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    Self::active_window().draw_frame();
    let event_loop = &mut unsafe { App::shared_mut() }.event_loop;
    let _ = event_loop.run_on_demand(App::event_loop_handle);
  }

  #[track_caller]
  unsafe fn shared_mut() -> &'static mut App {
    static mut INIT_ONCE: std::sync::Once = std::sync::Once::new();
    static mut APP: Option<App> = None;
    INIT_ONCE.call_once(|| {
      let event_loop = EventLoopBuilder::with_user_event().build().unwrap();
      let waker = EventWaker(event_loop.create_proxy());
      unsafe {
        AppCtx::set_clipboard(Box::new(crate::clipboard::Clipboard::new().unwrap()));
        AppCtx::set_runtime_waker(Box::new(waker));
      }
      register_platform_app_events_handlers();
      APP = Some(App {
        event_loop,
        events_stream: <_>::default(),
        active_wnd: None,
      })
    });
    AppCtx::thread_check();

    APP.as_mut().unwrap()
  }
}

impl EventSender {
  pub fn send(&self, e: AppEvent) {
    if let Err(err) = self.0.send_event(e) {
      log::error!("{}", err.to_string())
    }
  }
}

impl RuntimeWaker for EventWaker {
  fn clone_box(&self) -> Box<dyn RuntimeWaker + Send> { Box::new(self.clone()) }
  fn wake(&self) { let _ = self.0.send_event(AppEvent::FuturesWake); }
}

/// EventWaker only send `RibirEvent::FuturesWake`.
unsafe impl Send for EventWaker {}

pub(crate) fn request_redraw(wnd: &Window) {
  let wnd = wnd.shell_wnd().borrow();
  let shell = wnd.as_any().downcast_ref::<WinitShellWnd>().unwrap();
  shell.winit_wnd.request_redraw();
}

#[derive(Default)]
struct PreEditHandle(Option<String>);

impl PreEditHandle {
  const fn new() -> Self { Self(None) }
  fn update(&mut self, wnd: &Window, pre_edit: &Ime) {
    match pre_edit {
      Ime::Enabled => {}
      Ime::Preedit(txt, cursor) => match txt.is_empty() {
        true => self.exit(wnd),
        false => self.update_pre_edit(wnd, txt, cursor),
      },
      Ime::Commit(value) => {
        self.exit(wnd);
        wnd.processes_receive_chars(value.clone());
      }
      Ime::Disabled => self.exit(wnd),
    }
  }

  fn is_in_pre_edit(&self) -> bool { self.0.is_some() }

  fn force_exit(&mut self, wnd: &Window) {
    if self.is_in_pre_edit() {
      wnd.set_ime_allowed(false);
      wnd.processes_ime_pre_edit(ImePreEdit::End);
      if let Some(s) = self.0.take() {
        wnd.processes_receive_chars(s);
      }
      wnd.set_ime_allowed(true);
    }
  }

  fn exit(&mut self, wnd: &Window) {
    if self.is_in_pre_edit() {
      wnd.processes_ime_pre_edit(ImePreEdit::End);
      self.0.take();
    }
  }

  fn update_pre_edit(&mut self, wnd: &Window, txt: &str, cursor: &Option<(usize, usize)>) {
    if !self.is_in_pre_edit() {
      wnd.processes_ime_pre_edit(ImePreEdit::Begin);
    }

    wnd.processes_ime_pre_edit(ImePreEdit::PreEdit {
      value: txt.to_owned(),
      cursor: *cursor,
    });
    self.0 = Some(txt.to_owned());
  }
}

#[cfg(test)]
mod tests {
  use ribir_core::{
    prelude::*,
    test_helper::{MockBox, TestWindow},
  };
  use std::{cell::RefCell, rc::Rc};
  use winit::event::{Ime, WindowEvent};

  use super::App;
  #[derive(Debug, Default)]
  struct LogImeEvent {
    log: Rc<RefCell<Vec<String>>>,
  }
  impl Compose for LogImeEvent {
    fn compose(this: impl StateWriter<Value = Self>) -> impl WidgetBuilder {
      fn_widget! {
        @MockBox {
          size: INFINITY_SIZE,
          auto_focus: true,
          on_ime_pre_edit: move |e| {
            match &e.pre_edit {
              ImePreEdit::Begin => $this.log.borrow_mut().push("on_ime_pre_edit_begin".to_string()),
              ImePreEdit::PreEdit { value, .. } => $this.log.borrow_mut().push(format!("on_ime_pre_edit_update {value}")),
              ImePreEdit::End => $this.log.borrow_mut().push("on_ime_pre_edit_end".to_string()),
            }
          },
          on_chars: move|e| $this.log.borrow_mut().push(format!("on_chars {}", e.chars)),
          on_tap: move |_| $this.log.borrow_mut().push("on_tap".to_string()),
        }
      }
    }
  }
  #[test]
  fn ime_pre_edit() {
    let w = Stateful::new(LogImeEvent::default());
    let log = w.read().log.clone();

    let w = fn_widget! { @ {w} };
    let mut wnd = TestWindow::new_with_size(w, Size::new(200., 200.));

    wnd.draw_frame();

    App::dispatch_wnd_native_event(&wnd, WindowEvent::Ime(Ime::Enabled));
    App::dispatch_wnd_native_event(
      &wnd,
      WindowEvent::Ime(Ime::Preedit("hello".to_string(), None)),
    );
    App::dispatch_wnd_native_event(&wnd, WindowEvent::Ime(Ime::Disabled));
    wnd.draw_frame();
    assert_eq!(
      &*log.borrow(),
      &[
        "on_ime_pre_edit_begin",
        "on_ime_pre_edit_update hello",
        "on_ime_pre_edit_end"
      ]
    );

    log.borrow_mut().clear();
    App::dispatch_wnd_native_event(
      &wnd,
      WindowEvent::Ime(Ime::Preedit("hello".to_string(), None)),
    );
    App::dispatch_wnd_native_event(&wnd, WindowEvent::Ime(Ime::Commit("hello".to_string())));
    wnd.draw_frame();
    assert_eq!(
      &*log.borrow(),
      &[
        "on_ime_pre_edit_begin",
        "on_ime_pre_edit_update hello",
        "on_ime_pre_edit_end",
        "on_chars hello",
      ]
    );

    log.borrow_mut().clear();
    App::dispatch_wnd_native_event(
      &wnd,
      WindowEvent::Ime(Ime::Preedit("hello".to_string(), None)),
    );
    let device_id = unsafe { winit::event::DeviceId::dummy() };
    App::dispatch_wnd_native_event(
      &wnd,
      WindowEvent::MouseInput {
        state: winit::event::ElementState::Pressed,
        button: winit::event::MouseButton::Left,
        device_id,
      },
    );
    App::dispatch_wnd_native_event(
      &wnd,
      WindowEvent::MouseInput {
        state: winit::event::ElementState::Released,
        button: winit::event::MouseButton::Left,
        device_id,
      },
    );
    wnd.draw_frame();
    assert_eq!(
      &*log.borrow(),
      &[
        "on_ime_pre_edit_begin",
        "on_ime_pre_edit_update hello",
        "on_ime_pre_edit_end",
        "on_chars hello",
        "on_tap",
      ]
    );
  }
}
