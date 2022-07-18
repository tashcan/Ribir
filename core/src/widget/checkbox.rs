use crate::prelude::*;

/// Represents a control that a user can select and clear.
#[derive(Clone, Declare)]
pub struct Checkbox {
  #[declare(default)]
  pub checked: bool,
  #[declare(default)]
  pub indeterminate: bool,
  #[declare(default = "IconSize::of(ctx).tiny")]
  pub size: Size,
}

impl Checkbox {
  pub fn switch_check(&mut self) {
    if self.indeterminate {
      self.indeterminate = false;
      self.checked = false;
    } else {
      self.checked = !self.checked;
    }
  }
}

impl Compose for Checkbox {
  fn compose(this: Stateful<Self>, ctx: &mut BuildCtx) -> Widget {
    let icons = SvgIcons::of(ctx);
    let checked = icons.checked.clone();
    let unchecked = icons.unchecked.clone();
    let indeterminate = icons.indeterminate.clone();

    widget! {
      track { this }
      Icon {
        size: this.size,
        cursor: CursorIcon::Hand,
        on_tap: move |_| this.switch_check(),
        on_key_up: move |k| {
          if k.key == VirtualKeyCode::Space {
            this.switch_check()
          }
        },
        ExprWidget {
          expr: if this.indeterminate {
            indeterminate.clone()
          } else if this.checked {
            checked.clone()
          } else {
            unchecked.clone()
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test::widget_and_its_children_box_rect;

  #[test]
  fn layout() {
    let w = widget! { Checkbox {} };
    let (rect, _) = widget_and_its_children_box_rect(w.into_widget(), Size::new(200., 200.));
    debug_assert_eq!(rect, Rect::new(Point::new(0., 0.), Size::new(24., 24.)));
  }

  #[cfg(feature = "png")]
  #[test]
  fn checked_paint() {
    let c = widget! { Checkbox { checked: true } };
    let mut window = Window::wgpu_headless(c, DeviceSize::new(100, 100));
    window.render_ready();

    assert!(window.same_as_png("../test/test_imgs/checkbox_checked.png"));
  }

  #[cfg(feature = "png")]
  #[test]
  fn unchecked_paint() {
    let mut window = Window::wgpu_headless(widget! { Checkbox {} }, DeviceSize::new(100, 100));
    window.render_ready();
    assert!(window.same_as_png("../test/test_imgs/checkbox_uncheck.png"));
  }

  #[cfg(feature = "png")]
  #[test]
  fn indeterminate_paint() {
    let c = widget! {
      Checkbox {
        checked: true,
        indeterminate: true,
      }
    };
    let mut window = Window::wgpu_headless(c.into_widget(), DeviceSize::new(100, 100));
    window.render_ready();

    assert!(window.same_as_png("../test/test_imgs/checkbox_indeterminate.png"));

    let c = widget! {
      Checkbox {
        checked: false,
        indeterminate: true,
      }
    };
    let mut window = Window::wgpu_headless(c.into_widget(), DeviceSize::new(100, 100));
    window.render_ready();

    assert!(window.same_as_png("../test/test_imgs/checkbox_indeterminate.png"));
  }
}
