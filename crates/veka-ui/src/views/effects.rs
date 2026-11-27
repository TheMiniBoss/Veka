//! Effets qui lisent l'arrière-plan (étape 6) : flou d'arrière-plan et verre dépoli.

use veka_colora::{Paint, Token};
use veka_foundation::{Color, Rect};
use veka_graphics::{BackdropEffect, BlurEffect, Shape};

use crate::element::Element;
use crate::env::{BackdropQuality, Environment};
use crate::primitive::{PaintCx, Primitive};

/// Flou de ce qui est dessiné derrière la vue, découpé à `shape`, teinté (`.backdrop_blur`,
/// `.frosted_glass`).
#[derive(Clone, Debug, PartialEq)]
pub struct BackdropBlur {
    /// Rayon du flou, en points (comme le `blur()` du CSS).
    pub radius: f32,
    pub shape: Shape,
    /// Teinte mélangée au flou (un jeton Colora par défaut).
    pub tint: Paint,
    /// Part de la teinte, de 0 à 1.
    pub tint_amount: f32,
    /// 1 = inchangée ; au-dessus, couleurs plus vives.
    pub saturation: f32,
    /// Grain, de 0 à 1.
    pub noise: f32,
}

impl Default for BackdropBlur {
    fn default() -> Self {
        Self {
            radius: 20.0,
            shape: Shape::Rectangle,
            tint: Token::WindowBackground.into(),
            tint_amount: 0.15,
            saturation: 1.4,
            noise: 0.0,
        }
    }
}

impl BackdropBlur {
    /// Verre dépoli : flou fort, plus de teinte, un peu de grain.
    pub fn frosted() -> Self {
        Self {
            radius: 30.0,
            shape: Shape::RoundedRectangle(veka_graphics::CornerRadii::all(16.0)),
            tint_amount: 0.35,
            saturation: 1.6,
            noise: 0.035,
            ..Self::default()
        }
    }

    pub fn radius(self, radius: f32) -> Self {
        Self { radius, ..self }
    }

    pub fn shape(self, shape: Shape) -> Self {
        Self { shape, ..self }
    }

    pub fn tint(self, tint: impl Into<Paint>, amount: f32) -> Self {
        Self {
            tint: tint.into(),
            tint_amount: amount,
            ..self
        }
    }

    pub fn saturation(self, saturation: f32) -> Self {
        Self { saturation, ..self }
    }

    pub fn noise(self, noise: f32) -> Self {
        Self { noise, ..self }
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: 1.0,
    }
}

impl Primitive for BackdropBlur {
    fn name(&self) -> &'static str {
        "backdropBlur"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let env: &Environment = cx.env();
        let tint = env.resolve(&self.tint).average();
        let amount = self.tint_amount.clamp(0.0, 1.0);
        let quality = env.backdrop_quality;
        let fallback = (env.reduce_transparency || quality == BackdropQuality::Off)
            .then(|| env.resolve(&Paint::from(Token::WindowBackground)).average());
        if let Some(base) = fallback {
            // Repli : une teinte unie et opaque, sur le fond de la fenêtre.
            cx.list().fill(self.shape, rect, mix(base, tint, amount));
        } else {
            let effect = BackdropEffect::Blur(BlurEffect {
                radius: self.radius.max(0.0),
                tint,
                tint_amount: amount,
                saturation: self.saturation.max(0.0),
                noise: self.noise.clamp(0.0, 1.0),
                low_quality: quality == BackdropQuality::Low,
            });
            cx.list().backdrop(self.shape, rect, effect);
        }
        cx.paint_children();
    }

    fn animatable(&self, env: &Environment) -> Option<veka_anima::AnimatableData> {
        let mut d = veka_anima::AnimatableData::new();
        d.push(self.radius)
            .push(self.tint_amount)
            .push(self.saturation)
            .push(self.noise);
        super::anima::push_shape(&mut d, &self.shape);
        super::anima::push_paint(&mut d, &env.resolve_unadjusted(&self.tint));
        Some(d)
    }

    fn with_animatable(
        &self,
        env: &Environment,
        data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        let mut r = data.reader();
        let (radius, tint_amount, saturation, noise) = (r.f32(), r.f32(), r.f32(), r.f32());
        let shape = super::anima::read_shape(self.shape, &mut r);
        let tint = super::anima::read_paint(&env.resolve_unadjusted(&self.tint), &mut r);
        Some(std::rc::Rc::new(BackdropBlur {
            radius,
            shape,
            tint,
            tint_amount,
            saturation,
            noise,
        }))
    }
}

impl Element {
    /// Floute ce qui est dessiné derrière la vue (rectangle, rayon en points).
    pub fn backdrop_blur(self, radius: f32) -> Element {
        self.backdrop(BackdropBlur::default().radius(radius))
    }

    /// Verre dépoli : flou fort, teinte du fond de la fenêtre et un peu de grain, coins arrondis.
    pub fn frosted_glass(self) -> Element {
        self.backdrop(BackdropBlur::frosted())
    }

    /// Flou d'arrière-plan avec tous ses réglages.
    pub fn backdrop(self, blur: BackdropBlur) -> Element {
        self.wrap(blur)
    }
}
