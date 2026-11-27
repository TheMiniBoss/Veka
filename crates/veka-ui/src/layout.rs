//! Conteneurs de layout : `Stack` (H / V / Depth), `Spacer`, et les modifiers `padding` et `frame`.

use veka_foundation::{Insets, Point, Rect, Size};

use crate::primitive::{LayoutCx, Primitive, Proposal};
use crate::tree::NodeId;

/// Axe d'une pile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
    /// Les enfants sont superposés (ZStack).
    Depth,
}

/// Alignement sur un axe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    Leading,
    #[default]
    Center,
    Trailing,
}

impl Alignment {
    /// Position d'un objet de longueur `inner` dans `outer`, à partir de `start`.
    pub fn offset(self, start: f32, outer: f32, inner: f32) -> f32 {
        match self {
            Alignment::Leading => start,
            Alignment::Center => start + (outer - inner) * 0.5,
            Alignment::Trailing => start + outer - inner,
        }
    }
}

/// Espacement par défaut entre les enfants d'une pile.
pub const DEFAULT_SPACING: f32 = 8.0;
/// Longueur minimale par défaut d'un `Spacer`.
pub const DEFAULT_SPACER_MIN: f32 = 8.0;

fn main_of(axis: Axis, s: Size) -> f32 {
    if axis == Axis::Horizontal {
        s.width
    } else {
        s.height
    }
}
fn cross_of(axis: Axis, s: Size) -> f32 {
    if axis == Axis::Horizontal {
        s.height
    } else {
        s.width
    }
}
fn proposal_main(axis: Axis, p: Proposal) -> Option<f32> {
    if axis == Axis::Horizontal {
        p.width
    } else {
        p.height
    }
}
fn proposal_cross(axis: Axis, p: Proposal) -> Option<f32> {
    if axis == Axis::Horizontal {
        p.height
    } else {
        p.width
    }
}
fn make_size(axis: Axis, main: f32, cross: f32) -> Size {
    if axis == Axis::Horizontal {
        Size::new(main, cross)
    } else {
        Size::new(cross, main)
    }
}
fn make_proposal(axis: Axis, main: Option<f32>, cross: Option<f32>) -> Proposal {
    if axis == Axis::Horizontal {
        Proposal::new(main, cross)
    } else {
        Proposal::new(cross, main)
    }
}

/// Une pile : enfants les uns après les autres sur un axe, ou superposés.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub axis: Axis,
    pub spacing: f32,
    /// Alignement sur l'axe croisé (pour `Depth` : sur les deux axes).
    pub alignment: Alignment,
}

impl Stack {
    /// Répartit la place sur l'axe principal, comme SwiftUI : les enfants les moins flexibles
    /// d'abord, chacun recevant une part égale de ce qui reste.
    fn distribute(&self, cx: &mut LayoutCx<'_>, children: &[NodeId], p: Proposal) -> Vec<(NodeId, Size)> {
        let axis = self.axis;
        let cross = proposal_cross(axis, p);
        let n = children.len();
        let spacing = self.spacing * n.saturating_sub(1) as f32;
        let Some(main) = proposal_main(axis, p) else {
            return children
                .iter()
                .map(|&c| {
                    let s = cx.measure(c, make_proposal(axis, None, cross));
                    let s = if cx.child_name(c) == "Spacer" {
                        make_size(axis, main_of(axis, s), 0.0)
                    } else {
                        s
                    };
                    (c, s)
                })
                .collect();
        };
        // Comme SwiftUI, les Spacers passent après les autres enfants : on leur réserve leur longueur
        // minimale, puis ils se partagent ce qui reste.
        let spacer: Vec<bool> = children.iter().map(|&c| cx.child_name(c) == "Spacer").collect();
        let spacer_min: Vec<f32> = children
            .iter()
            .zip(&spacer)
            .map(|(&c, &is)| {
                if is {
                    main_of(axis, cx.measure(c, make_proposal(axis, Some(0.0), cross)))
                } else {
                    0.0
                }
            })
            .collect();
        let mut reserved: f32 = spacer_min.iter().sum();
        // Flexibilité = taille maximale - taille minimale sur l'axe principal.
        let mut order: Vec<(usize, f32)> = children
            .iter()
            .enumerate()
            .filter(|&(i, _)| !spacer[i])
            .map(|(i, &c)| {
                let lo = main_of(axis, cx.measure(c, make_proposal(axis, Some(0.0), cross)));
                let hi = main_of(
                    axis,
                    cx.measure(c, make_proposal(axis, Some(f32::INFINITY), cross)),
                );
                (i, hi - lo)
            })
            .collect();
        order.sort_by(|a, b| a.1.total_cmp(&b.1));
        order.extend((0..n).filter(|&i| spacer[i]).map(|i| (i, f32::INFINITY)));
        let others = n - spacer.iter().filter(|&&b| b).count();
        let mut remaining = (main - spacing).max(0.0);
        // Taille idéale de chaque enfant. Si toutes tiennent, chacun la reçoit d'abord et seul le surplus
        // est partagé : un texte ne passe pas à la ligne quand la pile a la place de l'écrire.
        let ideal: Vec<f32> = (0..n)
            .map(|i| {
                if spacer[i] {
                    0.0
                } else {
                    main_of(axis, cx.measure(children[i], make_proposal(axis, None, cross)))
                }
            })
            .collect();
        let fits = ideal.iter().sum::<f32>() + reserved <= remaining + 0.01;
        let mut ideal_left: f32 = if fits { ideal.iter().sum() } else { 0.0 };
        let mut sizes = vec![Size::ZERO; n];
        for (k, &(i, _)) in order.iter().enumerate() {
            let share = if spacer[i] {
                reserved -= spacer_min[i];
                (remaining / (n - k) as f32).max(spacer_min[i])
            } else if fits {
                ideal_left -= ideal[i];
                let extra = (remaining - reserved - ideal_left - ideal[i]).max(0.0);
                ideal[i] + extra / (others - k) as f32
            } else {
                ((remaining - reserved).max(0.0)) / (others - k) as f32
            };
            let mut s = cx.measure(children[i], make_proposal(axis, Some(share), cross));
            if spacer[i] {
                // Un Spacer ne prend aucune place sur l'axe croisé d'une pile.
                s = make_size(axis, main_of(axis, s), 0.0);
            }
            remaining = (remaining - main_of(axis, s)).max(0.0);
            sizes[i] = s;
        }
        children.iter().copied().zip(sizes).collect()
    }
}

impl Primitive for Stack {
    fn name(&self) -> &'static str {
        match self.axis {
            Axis::Horizontal => "HStack",
            Axis::Vertical => "VStack",
            Axis::Depth => "ZStack",
        }
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let children = cx.children();
        if self.axis == Axis::Depth {
            let mut s = Size::ZERO;
            for c in children {
                let m = cx.measure(c, p);
                s = Size::new(s.width.max(m.width), s.height.max(m.height));
            }
            return s;
        }
        let sizes = self.distribute(cx, &children, p);
        let main: f32 = sizes.iter().map(|(_, s)| main_of(self.axis, *s)).sum::<f32>()
            + self.spacing * sizes.len().saturating_sub(1) as f32;
        let cross = sizes
            .iter()
            .map(|(_, s)| cross_of(self.axis, *s))
            .fold(0.0, f32::max);
        if self.axis == Axis::Horizontal {
            Size::new(main, cross)
        } else {
            Size::new(cross, main)
        }
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        let children = cx.children();
        if self.axis == Axis::Depth {
            for c in children {
                let s = cx.measure(c, Proposal::exact(rect.size));
                let x = self.alignment.offset(rect.x(), rect.width(), s.width);
                let y = self.alignment.offset(rect.y(), rect.height(), s.height);
                cx.place(c, Rect::new(x, y, s.width, s.height));
            }
            return;
        }
        let sizes = self.distribute(cx, &children, Proposal::exact(rect.size));
        let mut pos = if self.axis == Axis::Horizontal {
            rect.x()
        } else {
            rect.y()
        };
        for (c, s) in sizes {
            let r = if self.axis == Axis::Horizontal {
                Rect::new(
                    pos,
                    self.alignment.offset(rect.y(), rect.height(), s.height),
                    s.width,
                    s.height,
                )
            } else {
                Rect::new(
                    self.alignment.offset(rect.x(), rect.width(), s.width),
                    pos,
                    s.width,
                    s.height,
                )
            };
            cx.place(c, r);
            pos += main_of(self.axis, s) + self.spacing;
        }
    }
}

/// Espace flexible qui pousse ses voisins dans une pile.
#[derive(Clone, Debug, PartialEq)]
pub struct Spacer {
    pub min_length: f32,
}

impl Primitive for Spacer {
    fn name(&self) -> &'static str {
        "Spacer"
    }

    fn size_that_fits(&self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        // Sans connaître l'axe de la pile, un Spacer s'étend sur les deux axes proposés et vaut 0 sur
        // l'axe croisé d'une pile (la pile prend le maximum de ses enfants, et l'axe croisé n'est
        // pas proposé à l'infini).
        let w = p
            .width
            .map(|w| if w.is_finite() { w.max(self.min_length) } else { w })
            .unwrap_or(self.min_length);
        let h = p
            .height
            .map(|h| if h.is_finite() { h.max(self.min_length) } else { h })
            .unwrap_or(self.min_length);
        Size::new(w.min(f32::MAX), h.min(f32::MAX))
    }
}

/// Marges autour du contenu.
#[derive(Clone, Debug, PartialEq)]
pub struct Padding {
    pub insets: Insets,
}

impl Primitive for Padding {
    fn name(&self) -> &'static str {
        "padding"
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let i = self.insets;
        let inner = Proposal::new(
            p.width.map(|w| (w - i.horizontal()).max(0.0)),
            p.height.map(|h| (h - i.vertical()).max(0.0)),
        );
        let mut s = Size::ZERO;
        for c in cx.children() {
            let m = cx.measure(c, inner);
            s = Size::new(s.width.max(m.width), s.height.max(m.height));
        }
        Size::new(s.width + i.horizontal(), s.height + i.vertical())
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        let inner = rect.inset(self.insets);
        for c in cx.children() {
            cx.place(c, inner);
        }
    }
}

/// Taille imposée (sur un axe ou les deux) ; le contenu est centré dedans.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub alignment: Alignment,
}

impl Primitive for Frame {
    fn name(&self) -> &'static str {
        "frame"
    }

    fn size_that_fits(&self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let inner = Proposal::new(self.width.or(p.width), self.height.or(p.height));
        let mut s = Size::ZERO;
        for c in cx.children() {
            let m = cx.measure(c, inner);
            s = Size::new(s.width.max(m.width), s.height.max(m.height));
        }
        Size::new(self.width.unwrap_or(s.width), self.height.unwrap_or(s.height))
    }

    fn place(&self, cx: &mut LayoutCx<'_>, rect: Rect) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::exact(rect.size));
            let x = self.alignment.offset(rect.x(), rect.width(), s.width);
            let y = self.alignment.offset(rect.y(), rect.height(), s.height);
            cx.place(c, Rect::from_origin_size(Point::new(x, y), s));
        }
    }
}
