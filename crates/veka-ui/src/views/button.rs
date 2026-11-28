use veka_foundation::{Insets, Rect, Size};
use veka_graphics::Shape;

use crate::element::Action;
use crate::env::Environment;
use crate::primitive::{EventCx, EventResult, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal};

/// Le primitive `Button`. Comportement (survol, appui, clic) séparé du look (style par défaut ici).
#[derive(Clone, Debug)]
pub struct ButtonPrim {
    pub action: Action,
}

/// État d'un bouton.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub hovered: bool,
    /// Appuyé et le pointeur est encore dans le bouton.
    pub pressed: bool,
    /// Appui en cours (même si le pointeur est sorti).
    pub tracking: bool,
}

const PADDING: Insets = Insets::new(8.0, 16.0, 8.0, 16.0);

impl Primitive for ButtonPrim {
    fn name(&self) -> &'static str {
        "Button"
    }

    fn primary_action(&self) -> Option<Action> {
        Some(self.action.clone())
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<ButtonState>::default()
    }

    fn update_env(&self, env: &mut Environment) -> bool {
        env.foreground = veka_colora::Token::ButtonText.into();
        if env.font_weight.is_none() {
            env.font_weight = Some(veka_text::Weight::SemiBold);
        }
        true
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let inner = Proposal::new(
            p.width.map(|w| (w - PADDING.horizontal()).max(0.0)),
            p.height.map(|h| (h - PADDING.vertical()).max(0.0)),
        );
        let mut s = Size::ZERO;
        for c in cx.children() {
            let m = cx.measure(c, inner);
            s = Size::new(s.width.max(m.width), s.height.max(m.height));
        }
        Size::new(s.width + PADDING.horizontal(), s.height + PADDING.vertical())
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        let inner = rect.inset(PADDING);
        for c in cx.children() {
            let s = cx.measure(c, crate::primitive::Proposal::exact(inner.size));
            cx.place(c, inner.centered(s));
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let st = *cx.state::<ButtonState>();
        let base = cx.env().resolve(&cx.env().accent);
        let dark = match (st.pressed, st.hovered) {
            (true, _) => 0.25,
            (false, true) => 0.1,
            _ => 0.0,
        };
        let paint = if dark > 0.0 {
            base.map_colors(|c| c.mix(veka_foundation::Color::Black.with_alpha(c.a), dark))
        } else {
            base
        };
        cx.list().fill(Shape::Capsule, rect, paint);
        cx.paint_children();
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Capsule)
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        let before = *cx.state::<ButtonState>();
        let st = cx.state::<ButtonState>();
        let mut action = None;
        match event {
            PointerEvent::Move { inside, .. } => {
                st.hovered = inside;
                st.pressed = st.tracking && inside;
            }
            PointerEvent::Down { .. } => {
                st.tracking = true;
                st.pressed = true;
                st.hovered = true;
            }
            PointerEvent::Up { inside, .. } => {
                if st.tracking && inside {
                    action = Some(self.action.clone());
                }
                st.tracking = false;
                st.pressed = false;
                st.hovered = inside;
            }
            PointerEvent::Leave => {
                st.hovered = false;
                st.pressed = false;
            }
        }
        let after = *st;
        EventResult {
            handled: true,
            repaint: before != after,
            action,
            animate: false,
        }
    }
}
