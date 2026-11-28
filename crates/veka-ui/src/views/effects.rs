//! Effets qui lisent l'arrière-plan (étape 6) : flou d'arrière-plan et verre dépoli.

use veka_colora::{Paint, Token};
use veka_foundation::{Color, Rect};
use veka_graphics::{
    BackdropEffect, BlendMode, BlurEffect, DrawCommand, GlassEffect, GlassShape, MAX_GLASS_SHAPES, Shape,
};

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
                ..BlurEffect::default()
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

// ---------------------------------------------------------------- Verre liquide

/// Le matériau d'un verre liquide (façon Liquid Glass 27). Partir de [`Glass::regular`] ou
/// [`Glass::clear`].
#[derive(Clone, Debug, PartialEq)]
pub struct Glass {
    /// De 0 (teinté : le fond se voit peu) à 1 (clair), comme le curseur de l'OS.
    pub clarity: f32,
    /// Teinte choisie (sinon celle du fond de la fenêtre, discrète).
    pub tint: Option<Paint>,
    /// Le reflet suit le pointeur.
    pub interactive: bool,
    /// Forme (sinon une capsule).
    pub shape: Option<Shape>,
    /// Déplacement du fond au bord, en points.
    pub refraction: f32,
    /// Largeur du bord courbe, en points.
    pub bezel: f32,
    /// Séparation des couleurs, de 0 à 1.
    pub aberration: f32,
    /// Reflet du bord, de 0 à 1.
    pub highlight: f32,
    /// Ombre portée, de 0 à 1.
    pub shadow: f32,
}

impl Glass {
    /// Le réglage « Par défaut » de 27 : un peu teinté, lisible sur tout fond.
    pub fn regular() -> Self {
        Self {
            clarity: 0.4,
            tint: None,
            interactive: false,
            shape: None,
            refraction: 22.0,
            bezel: 18.0,
            aberration: 0.3,
            highlight: 0.8,
            shadow: 0.2,
        }
    }

    /// Plus clair : le fond se voit nettement, seul le bord le tord.
    pub fn clear() -> Self {
        Self {
            clarity: 0.9,
            ..Self::regular()
        }
    }

    pub fn clarity(self, clarity: f32) -> Self {
        Self { clarity, ..self }
    }

    pub fn tint(self, tint: impl Into<Paint>) -> Self {
        Self {
            tint: Some(tint.into()),
            ..self
        }
    }

    pub fn interactive(self, interactive: bool) -> Self {
        Self { interactive, ..self }
    }

    pub fn shape(self, shape: Shape) -> Self {
        Self {
            shape: Some(shape),
            ..self
        }
    }

    pub fn refraction(self, refraction: f32) -> Self {
        Self { refraction, ..self }
    }

    pub fn shadow(self, shadow: f32) -> Self {
        Self { shadow, ..self }
    }

    /// Les réglages du renderer, couleurs résolues dans `env`.
    fn effect(&self, env: &Environment) -> GlassEffect {
        let c = self.clarity.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * c;
        let (tint, tint_amount) = match &self.tint {
            Some(p) => (env.resolve(p).average(), lerp(0.55, 0.22)),
            None => (
                env.resolve(&Paint::from(Token::WindowBackground)).average(),
                lerp(0.5, 0.04),
            ),
        };
        GlassEffect {
            blur: lerp(9.0, 1.5),
            refraction: self.refraction,
            bezel: self.bezel,
            aberration: self.aberration,
            highlight: self.highlight,
            rim_dark: lerp(0.25, 0.4),
            inner_light: lerp(0.35, 0.6),
            tint,
            tint_amount,
            saturation: lerp(1.15, 1.35),
            shadow: self.shadow,
            shadow_radius: 14.0,
            interactive: self.interactive,
            low_quality: env.backdrop_quality == BackdropQuality::Low,
            ..GlassEffect::default()
        }
    }

    /// Repli sans transparence : une teinte unie et opaque.
    fn fallback(&self, env: &Environment) -> Color {
        let base = env.resolve(&Paint::from(Token::WindowBackground)).average();
        match &self.tint {
            Some(p) => mix(base, env.resolve(p).average(), 0.35),
            None => base,
        }
    }
}

fn no_transparency(env: &Environment) -> bool {
    env.reduce_transparency || env.backdrop_quality == BackdropQuality::Off
}

/// Un verre liquide derrière la vue (`.glass()`). Dans un `glass_group`, il donne sa forme au groupe.
#[derive(Clone, Debug, PartialEq)]
pub struct GlassPrim {
    pub glass: Glass,
}

impl Primitive for GlassPrim {
    fn name(&self) -> &'static str {
        "glass"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let env: &Environment = cx.env();
        let shape = self.glass.shape.unwrap_or(Shape::Capsule);
        if env.in_glass_group {
            // Le groupe reprend la forme (verre ou teinte unie).
            cx.list().backdrop(shape, rect, BackdropEffect::GlassMember);
        } else if no_transparency(env) {
            let fill = self.glass.fallback(env);
            cx.list().fill(shape, rect, fill);
        } else {
            let effect = self.glass.effect(env);
            cx.list()
                .backdrop(shape, rect, BackdropEffect::Glass(Box::new(effect)));
        }
        cx.paint_children();
    }

    fn animatable(&self, _env: &Environment) -> Option<veka_anima::AnimatableData> {
        let mut d = veka_anima::AnimatableData::new();
        let g = &self.glass;
        d.push(g.clarity)
            .push(g.refraction)
            .push(g.bezel)
            .push(g.aberration)
            .push(g.highlight)
            .push(g.shadow);
        Some(d)
    }

    fn with_animatable(
        &self,
        _env: &Environment,
        data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        let mut r = data.reader();
        let glass = Glass {
            clarity: r.f32(),
            refraction: r.f32(),
            bezel: r.f32(),
            aberration: r.f32(),
            highlight: r.f32(),
            shadow: r.f32(),
            ..self.glass.clone()
        };
        Some(std::rc::Rc::new(GlassPrim { glass }))
    }
}

/// Un seul verre pour tous les `.glass()` du sous-arbre : leurs formes fusionnent comme des gouttes
/// quand elles se rapprochent à moins de `smoothness` points (`.glass_group`).
#[derive(Clone, Debug, PartialEq)]
pub struct GlassGroup {
    pub smoothness: f32,
    pub glass: Glass,
}

impl Primitive for GlassGroup {
    fn name(&self) -> &'static str {
        "glassGroup"
    }

    fn update_env(&self, env: &mut Environment) -> bool {
        let changed = !env.in_glass_group;
        env.in_glass_group = true;
        changed
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _rect: Rect) {
        let start = cx.list().commands.len();
        cx.paint_children();
        let env: &Environment = cx.env();
        if no_transparency(env) {
            // Les formes ont été remplies d'une teinte unie par le groupe, à leur place.
            let fill = self.glass.fallback(env);
            let shapes = collect_members(cx.list(), start);
            let list = cx.list();
            for (i, s) in shapes.into_iter().enumerate() {
                list.commands.insert(
                    start + i,
                    veka_graphics::DrawCommand::FillShape {
                        shape: s.shape,
                        rect: s.rect,
                        paint: fill.into(),
                    },
                );
            }
            return;
        }
        let mut effect = self.glass.effect(env);
        let shapes = collect_members(cx.list(), start);
        if shapes.is_empty() {
            return;
        }
        let n = shapes.len().min(MAX_GLASS_SHAPES);
        let mut bounds = shapes[0].rect;
        for (i, s) in shapes.iter().take(n).enumerate() {
            effect.shapes[i] = *s;
            bounds = bounds.union(&s.rect);
        }
        effect.count = n as u8;
        effect.smoothness = self.smoothness.max(0.0);
        cx.list().commands.insert(
            start,
            veka_graphics::DrawCommand::Backdrop {
                shape: Shape::Rectangle,
                rect: bounds,
                effect: BackdropEffect::Glass(Box::new(effect)),
            },
        );
    }

    fn animatable(&self, _env: &Environment) -> Option<veka_anima::AnimatableData> {
        let mut d = veka_anima::AnimatableData::new();
        d.push(self.smoothness).push(self.glass.clarity);
        Some(d)
    }

    fn with_animatable(
        &self,
        _env: &Environment,
        data: &veka_anima::AnimatableData,
    ) -> Option<std::rc::Rc<dyn Primitive>> {
        let mut r = data.reader();
        let smoothness = r.f32();
        let clarity = r.f32();
        Some(std::rc::Rc::new(GlassGroup {
            smoothness,
            glass: Glass {
                clarity,
                ..self.glass.clone()
            },
        }))
    }
}

/// Retire les formes des verres du groupe (depuis `start`) et les renvoie dans le repère du groupe
/// (les `offset` et `scale_effect` des enfants appliqués).
fn collect_members(list: &mut veka_graphics::DrawList, start: usize) -> Vec<GlassShape> {
    use veka_foundation::{Point, Transform2D};
    use veka_graphics::DrawCommand;
    let mut out = Vec::new();
    let mut stack = vec![Transform2D::IDENTITY];
    let mut keep = Vec::with_capacity(list.commands.len() - start);
    for cmd in list.commands.drain(start..) {
        match &cmd {
            DrawCommand::PushTransform(m) => {
                let t = *stack.last().expect("pile");
                stack.push(m.then(&t));
            }
            DrawCommand::PopTransform => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            DrawCommand::Backdrop {
                shape,
                rect,
                effect: BackdropEffect::GlassMember,
            } => {
                let t = *stack.last().expect("pile");
                let a = t.apply(rect.origin);
                let b = t.apply(Point::new(rect.max_x(), rect.max_y()));
                let r = Rect::new(a.x.min(b.x), a.y.min(b.y), (b.x - a.x).abs(), (b.y - a.y).abs());
                out.push(GlassShape {
                    shape: *shape,
                    rect: r,
                });
                continue;
            }
            _ => {}
        }
        keep.push(cmd);
    }
    list.commands.extend(keep);
    out
}

/// Ondulation du fond dans la forme (`.liquid_distortion`). Le temps vient d'une horloge.
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidDistortion {
    /// Déplacement du fond, en points.
    pub strength: f32,
    /// Taille des vagues, en points.
    pub scale: f32,
    /// 1 = normal, 0 = figé.
    pub speed: f32,
    pub shape: Shape,
    /// Temps, en secondes (donné par l'horloge).
    pub time: f32,
}

impl Primitive for LiquidDistortion {
    fn name(&self) -> &'static str {
        "liquidDistortion"
    }

    fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        let env: &Environment = cx.env();
        if !no_transparency(env) {
            let time = if env.reduces_motion() {
                0.0
            } else {
                self.time * self.speed
            };
            let effect = BackdropEffect::Blur(BlurEffect {
                radius: 0.0,
                tint_amount: 0.0,
                saturation: 1.0,
                distortion: self.strength.max(0.0),
                wave: self.scale.max(1.0),
                time,
                ..BlurEffect::default()
            });
            cx.list().backdrop(self.shape, rect, effect);
        }
        cx.paint_children();
    }
}

impl Element {
    /// Verre liquide (matériau `regular`, capsule).
    pub fn glass(self) -> Element {
        self.glass_with(Glass::regular())
    }

    /// Verre liquide avec son matériau.
    pub fn glass_with(self, glass: Glass) -> Element {
        self.wrap(GlassPrim { glass })
    }

    /// Fusionne les `.glass()` du sous-arbre en un seul verre (matériau `regular`).
    pub fn glass_group(self, smoothness: f32) -> Element {
        self.glass_group_with(smoothness, Glass::regular())
    }

    /// Comme [`glass_group`](Self::glass_group), avec le matériau du groupe.
    pub fn glass_group_with(self, smoothness: f32, glass: Glass) -> Element {
        self.wrap(GlassGroup { smoothness, glass })
    }

    /// Le fond ondule dans la forme (eau, chaleur). Une horloge redemande des images tant que la vue
    /// est là (figée si le système réduit les animations).
    pub fn liquid_distortion(self, strength: f32, scale: f32, speed: f32, shape: Shape) -> Element {
        let child = self;
        let tag = (strength.to_bits() as u64) << 32 | scale.to_bits() as u64;
        crate::element::component_tagged(tag ^ 0x6c69_7175_6964, "liquidDistortion", move || {
            let clock = crate::timeline::TimelineState::playing(
                veka_anima::Timeline::new()
                    .track(
                        "t",
                        [
                            // La courbe d'une clé adoucit le segment qui en part : linéaire dès 0.
                            veka_anima::Keyframe::new(0.0, 0.0).curve(veka_anima::Curve::Linear),
                            veka_anima::Keyframe::new(1.0e6, 1.0e6),
                        ],
                    )
                    .length(1.0e6),
            );
            child.clone().wrap(LiquidDistortion {
                strength,
                scale,
                speed,
                shape,
                time: clock.value("t"),
            })
        })
    }
}

/// Un calque (étape 8) : le sous-arbre est dessiné à part, puis composé d'un bloc sur ce qui est
/// derrière (`.blend_mode`, `.compositing_group`).
#[derive(Clone, Debug, PartialEq)]
pub struct CompositingGroup {
    pub blend: BlendMode,
}

impl Primitive for CompositingGroup {
    fn name(&self) -> &'static str {
        if self.blend == BlendMode::Normal {
            "compositingGroup"
        } else {
            "blendMode"
        }
    }

    fn paint(&self, cx: &mut PaintCx<'_>, _rect: Rect) {
        cx.list().push(DrawCommand::PushGroup {
            blend: self.blend,
            opacity: 1.0,
        });
        cx.paint_children();
        cx.list().push(DrawCommand::PopGroup);
    }
}

impl Element {
    /// Mélange la vue avec ses frères dessinés derrière elle (`multiply`, `screen`…), d'un bloc : comme
    /// SwiftUI, le sous-arbre est d'abord dessiné à part. Le conteneur des frères est isolé : le fond de
    /// la fenêtre et ce qui est derrière le conteneur ne sont pas mélangés.
    pub fn blend_mode(self, blend: BlendMode) -> Element {
        self.wrap(CompositingGroup { blend })
    }

    /// Dessine le sous-arbre à part avant de le composer : une `.opacity` appliquée ensuite rend le
    /// groupe transparent d'un bloc (les recouvrements ne se voient plus).
    pub fn compositing_group(self) -> Element {
        self.wrap(CompositingGroup {
            blend: BlendMode::Normal,
        })
    }
}
