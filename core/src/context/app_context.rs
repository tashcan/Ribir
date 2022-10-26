use std::{
  cell::RefCell,
  rc::Rc,
  sync::{Arc, RwLock},
  time::Instant,
};

use crate::ticker::FrameTicker;
use crate::{builtin_widgets::Theme, ticker::FrameMsg};
use ::text::shaper::TextShaper;
pub use futures::task::SpawnError;
use futures::{
  executor::{block_on, LocalPool},
  task::LocalSpawnExt,
  Future,
};
use text::{font_db::FontDB, TextReorder, TypographyStore};

#[derive(Clone)]
pub struct AppContext {
  pub app_theme: Rc<Theme>,
  pub font_db: Arc<RwLock<FontDB>>,
  pub shaper: TextShaper,
  pub reorder: TextReorder,
  pub typography_store: TypographyStore,
  pub frame_ticker: FrameTicker,
  pub executor: Executor,
}

#[derive(Clone)]
pub struct Executor {
  #[cfg(feature = "thread-pool")]
  pub thread_pool: futures::executor::ThreadPool,
  pub local: Rc<RefCell<LocalPool>>,
}

impl AppContext {
  pub fn begin_frame(&mut self) { self.frame_ticker.emit(FrameMsg::NewFrame(Instant::now())); }
  pub fn layout_ready(&mut self) {
    self
      .frame_ticker
      .emit(FrameMsg::LayoutReady(Instant::now()));
  }

  pub fn end_frame(&mut self) {
    // todo: frame cache is not a good choice? because not every text will relayout
    // in every frame.
    self.shaper.end_frame();
    self.reorder.end_frame();
    self.typography_store.end_frame();

    self.frame_ticker.emit(FrameMsg::Finish(Instant::now()));
  }
}

impl Default for AppContext {
  fn default() -> Self {
    let mut font_db = FontDB::default();
    font_db.load_system_fonts();
    let font_db = Arc::new(RwLock::new(font_db));
    let shaper = TextShaper::new(font_db.clone());
    let reorder = TextReorder::default();
    let typography_store = TypographyStore::new(reorder.clone(), font_db.clone(), shaper.clone());
    let frame_ticker = FrameTicker::default();

    AppContext {
      font_db: <_>::default(),
      app_theme: <_>::default(),
      shaper,
      reorder,
      typography_store,
      frame_ticker,
      executor: <_>::default(),
    }
  }
}

impl Default for Executor {
  fn default() -> Self {
    Self {
      #[cfg(feature = "thread-pool")]
      thread_pool: futures::executor::ThreadPool::new().unwrap(),
      local: Default::default(),
    }
  }
}

impl Executor {
  #[inline]
  pub fn spawn_local<Fut>(&self, future: Fut) -> Result<(), SpawnError>
  where
    Fut: Future<Output = ()> + 'static,
  {
    self.local.borrow().spawner().spawn_local(future)
  }

  #[cfg(feature = "thread-pool")]
  #[inline]
  pub fn spawn_in_pool<Fut>(&self, future: Fut)
  where
    Fut: 'static + Future<Output = ()> + Send,
  {
    self.thread_pool.spawn_ok(future)
  }
}

impl AppContext {
  pub fn wait_future<F: Future>(f: F) -> F::Output { block_on(f) }
}