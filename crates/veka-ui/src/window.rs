//! Les fenêtres dessinées par Veka : zones de déplacement, boutons de fenêtre, barre de titre.
//!
//! Le cœur ne parle pas au système : il décrit des **régions** (voir [`Surface::regions`]) et
//! met les **actions** demandées (réduire, agrandir…) dans une file que l'hôte vide
//! ([`Surface::take_window_actions`]).
//!
//! [`Surface::regions`]: crate::Surface::regions
//! [`Surface::take_window_actions`]: crate::Surface::take_window_actions

#![allow(non_snake_case)]

use std::cell::RefCell;

use veka_foundation::{Color, Rect, RegionKind, Size, Transform2D, WindowAction, WindowControl};
use veka_graphics::{DrawCommand, Shape};

use crate::element::{Action, Element};
use crate::env::{Environment, Font};
use crate::layout::{Axis, Spacer};
use crate::primitive::{EventCx, EventResult, LayoutCx, PaintCx, PointerEvent, Primitive, Proposal};
use crate::views::{Text, stack};

thread_local! {
    static ACTIONS: RefCell<Vec<WindowAction>> = const { RefCell::new(Vec::new()) };
}

/// Demande une action sur la fenêtre de la vue en cours (pendant un événement ou une action).
pub fn perform_window_action(action: WindowAction) {
    ACTIONS.with(|a| a.borrow_mut().push(action));
}

pub(crate) fn take_actions() -> Vec<WindowAction> {
    ACTIONS.with(|a| std::mem::take(&mut *a.borrow_mut()))
}

/// `WindowAction::Minimize.perform()`.
pub trait WindowActionExt {
    fn perform(self);
}

impl WindowActionExt for WindowAction {
    fn perform(self) {
        perform_window_action(self);
    }
}

// --- Zone de déplacement -----------------------------------------------------------------------

/// `.window_drag_area()` : la zone déplace la fenêtre.
#[derive(Clone, Debug)]
pub struct WindowDragArea;

impl Primitive for WindowDragArea {
    fn name(&self) -> &'static str {
        "windowDragArea"
    }

    fn window_region(&self) -> Option<RegionKind> {
        Some(RegionKind::Drag)
    }
}

// --- Bouton de fenêtre sur n'importe quelle vue ------------------------------------------------

/// `.window_control(c)` : la vue devient le bouton `c` de la fenêtre. Ses enfants ne reçoivent
/// plus le pointeur.
#[derive(Clone, Debug)]
pub struct WindowControlArea {
    pub control: WindowControl,
}

/// Survol et appui d'un bouton de fenêtre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowButtonState {
    pub hovered: bool,
    pub pressed: bool,
    pub tracking: bool,
}

fn button_pointer(control: WindowControl, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
    let st = cx.state::<WindowButtonState>();
    let before = *st;
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
                action = Some(Action::new(move || perform_window_action(control.action())));
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
    }
}

impl Primitive for WindowControlArea {
    fn name(&self) -> &'static str {
        "windowControl"
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<WindowButtonState>::default()
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(Shape::Rectangle)
    }

    fn hit_children(&self) -> bool {
        false
    }

    fn window_region(&self) -> Option<RegionKind> {
        Some(RegionKind::Control(self.control))
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        button_pointer(self.control, cx, event)
    }
}

impl Element {
    /// La vue déplace la fenêtre quand on la tire (double-clic : agrandir / restaurer).
    /// Les vues interactives qu'elle contient (boutons…) restent cliquables.
    pub fn window_drag_area(self) -> Element {
        self.wrap(WindowDragArea)
    }

    /// La vue devient le bouton `control` de la fenêtre.
    pub fn window_control(self, control: WindowControl) -> Element {
        self.wrap(WindowControlArea { control })
    }
}

// --- Boutons de la barre prête à l'emploi -----------------------------------------------------

/// Style des boutons et de la barre : celui de Windows (à droite) ou de macOS (feux, à gauche).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleBarStyle {
    Windows,
    Mac,
}

impl TitleBarStyle {
    /// Le style du système pour lequel Veka est compilé.
    pub const fn native() -> Self {
        if cfg!(target_os = "macos") {
            TitleBarStyle::Mac
        } else {
            TitleBarStyle::Windows
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "windows" => Some(TitleBarStyle::Windows),
            "mac" => Some(TitleBarStyle::Mac),
            _ => None,
        }
    }
}

/// Un bouton de fenêtre dessiné par Veka (icône de Windows ou feu de macOS).
#[derive(Clone, Debug)]
pub struct WindowButtonPrim {
    pub control: WindowControl,
    pub style: TitleBarStyle,
}

const WIN_BUTTON: Size = Size::new(46.0, 32.0);
const MAC_LIGHT: f32 = 12.0;

impl Primitive for WindowButtonPrim {
    fn name(&self) -> &'static str {
        "WindowButton"
    }

    fn create_state(&self) -> Box<dyn std::any::Any> {
        Box::<WindowButtonState>::default()
    }

    fn size_that_fits(&self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        match self.style {
            TitleBarStyle::Windows => WIN_BUTTON,
            TitleBarStyle::Mac => Size::new(MAC_LIGHT, MAC_LIGHT),
        }
    }

    fn hit_shape(&self) -> Option<Shape> {
        Some(match self.style {
            TitleBarStyle::Windows => Shape::Rectangle,
            TitleBarStyle::Mac => Shape::Circle,
        })
    }

    fn window_region(&self) -> Option<RegionKind> {
        Some(RegionKind::Control(self.control))
    }

    fn pointer(&self, cx: &mut EventCx<'_>, event: PointerEvent) -> EventResult {
        button_pointer(self.control, cx, event)
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let st = *cx.state::<WindowButtonState>();
        let env = cx.env().clone();
        match self.style {
            TitleBarStyle::Windows => paint_windows_button(cx, rect, self.control, st, &env),
            TitleBarStyle::Mac => paint_mac_light(cx, rect, self.control, st, &env),
        }
    }
}

fn paint_windows_button(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    control: WindowControl,
    st: WindowButtonState,
    env: &Environment,
) {
    let close = control == WindowControl::Close;
    let background = match (close, st.pressed, st.hovered) {
        (true, true, _) => Some(Color::hex(0xF1707A)),
        (true, false, true) => Some(Color::hex(0xE81123)),
        (false, true, _) => Some(env.foreground.with_alpha(0.12)),
        (false, false, true) => Some(env.foreground.with_alpha(0.07)),
        _ => None,
    };
    if let Some(b) = background {
        cx.list().fill(Shape::Rectangle, rect, b);
    }
    let ink = if close && st.hovered {
        Color::White
    } else {
        env.foreground
    };
    let c = rect.center();
    let list = cx.list();
    match control {
        WindowControl::Minimize => {
            list.fill(Shape::Rectangle, Rect::new(c.x - 5.0, c.y - 0.5, 10.0, 1.0), ink);
        }
        WindowControl::Maximize | WindowControl::Fullscreen => {
            if env.window.maximized || env.window.fullscreen {
                // Restaurer : deux carrés décalés.
                list.stroke(
                    Shape::Rectangle,
                    Rect::new(c.x - 3.0, c.y - 5.0, 8.0, 8.0),
                    ink,
                    1.0,
                );
                list.fill(
                    Shape::Rectangle,
                    Rect::new(c.x - 5.0, c.y - 3.0, 8.0, 8.0),
                    background.unwrap_or(env.background),
                );
                list.stroke(
                    Shape::Rectangle,
                    Rect::new(c.x - 5.0, c.y - 3.0, 8.0, 8.0),
                    ink,
                    1.0,
                );
            } else {
                list.stroke(
                    Shape::Rectangle,
                    Rect::new(c.x - 5.0, c.y - 5.0, 10.0, 10.0),
                    ink,
                    1.0,
                );
            }
        }
        WindowControl::Close => {
            for angle in [45.0_f32, -45.0] {
                let t = Transform2D::translation(-c.x, -c.y)
                    .then(&Transform2D::rotation(angle.to_radians()))
                    .then(&Transform2D::translation(c.x, c.y));
                list.push(DrawCommand::PushTransform(t));
                list.fill(Shape::Rectangle, Rect::new(c.x - 7.0, c.y - 0.5, 14.0, 1.0), ink);
                list.push(DrawCommand::PopTransform);
            }
        }
    }
}

fn paint_mac_light(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    control: WindowControl,
    st: WindowButtonState,
    env: &Environment,
) {
    let base = if env.window.active || st.hovered {
        match control {
            WindowControl::Close => Color::hex(0xFF5F57),
            WindowControl::Minimize => Color::hex(0xFEBC2E),
            WindowControl::Maximize | WindowControl::Fullscreen => Color::hex(0x28C840),
        }
    } else {
        Color::hex(0xDDDDDD)
    };
    let color = if st.pressed {
        base.mix(Color::Black, 0.2)
    } else {
        base
    };
    cx.list().fill(Shape::Circle, rect, color);
    cx.list()
        .stroke(Shape::Circle, rect, Color::Black.with_alpha(0.12), 0.5);
}

/// Un bouton de fenêtre au style du système.
pub fn WindowButton(control: WindowControl) -> Element {
    WindowButtonWith(control, TitleBarStyle::native())
}

/// Un bouton de fenêtre au style donné.
pub fn WindowButtonWith(control: WindowControl, style: TitleBarStyle) -> Element {
    Element::new(WindowButtonPrim { control, style }, vec![])
}

// --- Barre de titre ---------------------------------------------------------------------------

/// Le fond et le comportement de la barre de titre : zone de déplacement, texte atténué quand la
/// fenêtre n'est pas active.
#[derive(Clone, Debug)]
pub struct TitleBarPrim {
    pub style: TitleBarStyle,
}

impl Primitive for TitleBarPrim {
    fn name(&self) -> &'static str {
        "TitleBar"
    }

    fn update_env(&self, env: &mut Environment) -> bool {
        if !env.window.active {
            env.foreground = env.foreground.mix(env.background, 0.55);
        }
        env.font = Font::Footnote;
        true
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let height = match self.style {
            TitleBarStyle::Windows => WIN_BUTTON.height,
            TitleBarStyle::Mac => 28.0,
        };
        for c in cx.children() {
            cx.measure(c, Proposal::new(p.width, Some(height)));
        }
        Size::new(p.width.unwrap_or(0.0), height)
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        for c in cx.children() {
            cx.place(c, rect);
        }
    }

    fn window_region(&self) -> Option<RegionKind> {
        Some(RegionKind::Drag)
    }
}

/// Une barre de titre prête à l'emploi : titre et boutons placés comme sur le système
/// (à droite sur Windows, feux à gauche sur macOS). Elle déplace la fenêtre.
pub fn TitleBar(title: impl Into<String>) -> Element {
    TitleBarWith(title, TitleBarStyle::native())
}

/// Comme [`TitleBar`], avec un style imposé.
pub fn TitleBarWith(title: impl Into<String>, style: TitleBarStyle) -> Element {
    let title = title.into();
    let content = match style {
        TitleBarStyle::Windows => stack(
            Axis::Horizontal,
            Some(0.0),
            vec![
                Text(title).padding_hv(12.0, 0.0),
                Element::new(Spacer { min_length: 0.0 }, vec![]),
                WindowButtonWith(WindowControl::Minimize, style),
                WindowButtonWith(WindowControl::Maximize, style),
                WindowButtonWith(WindowControl::Close, style),
            ],
        ),
        TitleBarStyle::Mac => stack(
            Axis::Depth,
            None,
            vec![
                Text(title),
                stack(
                    Axis::Horizontal,
                    Some(8.0),
                    vec![
                        WindowButtonWith(WindowControl::Close, style),
                        WindowButtonWith(WindowControl::Minimize, style),
                        WindowButtonWith(WindowControl::Maximize, style),
                        Element::new(Spacer { min_length: 0.0 }, vec![]),
                    ],
                )
                .padding_hv(12.0, 0.0),
            ],
        ),
    };
    Element::new(TitleBarPrim { style }, vec![content])
}
