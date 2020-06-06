// use super::box_constraint::BoxBound;
use super::flex::FlexFit;
use crate::prelude::{Point, Size};
use crate::render::render_ctx::RenderCtx;
use crate::render::render_tree::*;
use crate::render::*;
use crate::widget::Widget;

// Expand box in the layout container will auto fill the area.
#[derive(Debug)]
struct ExpandBox {
  flex: i32,
  fit: FlexFit,
  child: Box<dyn Widget>,
}
#[derive(Debug)]
struct ExpandBoxRender {
  flex: i32,
  fit: FlexFit,

  size: Option<Size>,
}

impl RenderWidget for ExpandBox {
  type RO = ExpandBoxRender;
  fn create_render_object(&self) -> Self::RO {
    ExpandBoxRender {
      flex: self.flex,
      fit: self.fit,
      size: None,
    }
  }
}

pub trait FlexElem {
  fn flex(&self) -> Option<i32>;
  fn fit(&self) -> Option<FlexFit>;
}

impl<O> FlexElem for O {
  default fn flex(&self) -> Option<i32> { None }
  default fn fit(&self) -> Option<FlexFit> { None }
}

impl FlexElem for ExpandBox {
  fn flex(&self) -> Option<i32> { Some(self.flex) }
  fn fit(&self) -> Option<FlexFit> { Some(self.fit) }
}

impl RenderObject<ExpandBox> for ExpandBoxRender {
  fn update(&mut self, owner: &ExpandBox) {
    self.fit = owner.fit;
    self.flex = owner.flex;
    self.size = None;
  }
  fn perform_layout(&mut self, _id: RenderId, _ctx: &mut RenderCtx) {}
  fn get_size(&self) -> Option<Size> { return self.size.clone(); }
  fn get_constraints(&self) -> LayoutConstraints { return LayoutConstraints::EFFECTED_BY_PARENT; }
  fn set_box_limit(&mut self, _bound: Option<BoxLimit>) {}

  fn paint<'a>(&'a self, ctx: &mut PaintingContext<'a>) {}
  fn child_offset(&self, idx: usize) -> Option<Point> { None }
}