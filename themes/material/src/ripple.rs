use super::state_layer::StateRole;
use ribir_core::prelude::*;
use ribir_widgets::prelude::*;

/// Widget use to do ripple animate as a visual feedback to user interactive.
/// Usually for touch and mouse.
#[derive(Declare, Debug)]
pub struct Ripple {
  /// The color of ripples.
  pub color: Color,
  /// The radius in pixels of foreground ripples when fully expanded. The
  /// default radius will be the distance from the center of the ripple to the
  /// furthest corner of the host bounding rectangle.
  #[declare(default, convert=strip_option)]
  pub radius: Option<f32>,
  /// Whether the ripple always originates from the center of the host bound.
  #[declare(default)]
  pub center: bool,
  #[declare(default=RippleBound::Bounded)]
  /// How ripples show outside of the host widget box.
  pub bounded: RippleBound,
  /// The position of current animate launch start.
  #[declare(skip)]
  launch_pos: Option<Point>,
}

/// Config how ripples show outside of the host widget box.
#[derive(Debug, PartialEq)]
pub enum RippleBound {
  /// Ripples visible outside of the host widget.
  Unbounded,
  /// Ripples only visible in the host widget box.
  Bounded,
  /// Ripples only visible in the host widget box with a border radius.
  Radius(Radius),
}

impl ComposeChild for Ripple {
  type Child = Widget;

  fn compose_child(this: State<Self>, child: Self::Child) -> Widget {
    widget! {
      states { this: this.into_writable() }
      init ctx => {
        let linear_transition = transitions::LINEAR.of(ctx);
        let ease_out = transitions::EASE_OUT.of(ctx);
      }
      Stack {
        id: container,
        fit: StackFit::Passthrough,
        on_pointer_down: move |e| this.launch_pos = if this.center {
          let center = container.layout_size() / 2.;
          Some(Point::new(center.width, center.height))
        } else {
          Some(e.position())
        },
        widget::from(child)
        Option::map(this.launch_pos, |launch_at| {
          let radius = this.radius.unwrap_or_else(|| {
            let size = container.layout_size();
            let distance_x = f32::max(launch_at.x , size.width - launch_at.x);
            let distance_y = f32::max(launch_at.y, size.height - launch_at.y);
            (distance_x.powf(2.) + distance_y.powf(2.)).sqrt()
          });
          let linear_transition = linear_transition.clone();
          let ease_out = ease_out.clone();

          widget!{
            IgnorePointer {
              id: ripple,
              opacity: 1.,
              delay_drop_until: !ripper_fade_out.is_running(),
              on_disposed: move |_| ripper_fade_out.run(),
              DynWidget {
                dyns: widget::then(this.bounded != RippleBound::Unbounded, || {
                  let rect = Rect::from_size(container.layout_size());
                  let path = match this.bounded {
                    RippleBound::Unbounded => unreachable!(),
                    RippleBound::Bounded => PaintPath::rect(&rect),
                    RippleBound::Radius(radius) => {
                      PaintPath::rect_round(&rect, &radius)
                    }
                  };
                  Clip { clip: ClipType::Path(path) }
                }),
                Container {
                  size: container.layout_size(),
                  PathPaintKit {
                    id: ripple_path,
                    brush: StateRole::pressed().calc_color(this.color),
                    path: PaintPath::circle(launch_at, radius),
                    on_mounted: move |_| { ripper_enter.run(); }
                  }
                }
              }
            }
            Animate {
              id: ripper_enter,
              transition: linear_transition,
              prop: prop!(ripple_path.path, move |_, _, rate| {
                let radius = Lerp::lerp(&0., &radius, rate);
                let center = this.launch_pos.clone().unwrap();
                PaintPath::circle(center, radius)
              }),
              from: PaintPath::circle(Point::zero(), 0.)
            }
            Animate {
              id: ripper_fade_out,
              from: 0.,
              prop: prop!(ripple.opacity, |from, to, rate| to.lerp(from,rate)),
              transition: ease_out,
            }
            finally {
              let_watch!(!container.pointer_pressed() && !ripper_enter.is_running())
              .filter(|b| *b)
              .subscribe(move |_| {
                this.launch_pos.take();
              });
            }
          }
        })
      }
    }
  }
}

impl Ripple {
  /// Manual launch a ripple animate at `pos`.
  pub fn launch_at(&mut self, pos: Point) { self.launch_pos = Some(pos); }
}
