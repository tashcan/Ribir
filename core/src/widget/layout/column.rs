use super::flex::*;
use crate::prelude::*;

#[derive(Default, Declare, Clone)]
pub struct Column {
  #[declare(default)]
  pub reverse: bool,
  #[declare(default)]
  pub wrap: bool,
  #[declare(default)]
  pub align_items: Align,
  #[declare(default)]
  pub justify_content: JustifyContent,
}

impl ComposeMultiChild for Column {
  fn compose_multi_child(this: StateWidget<Self>, children: Vec<Widget>) -> Widget {
    widget_try_track! {
      try_track { this }
      Flex {
        reverse: this.reverse,
        wrap: this.wrap,
        direction: Direction::Vertical,
        align_items: this.align_items,
        justify_content: this.justify_content,
        ExprWidget {
          expr: children.into_iter()
        }
      }
    }
  }
}
