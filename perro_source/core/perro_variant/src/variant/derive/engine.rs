use super::super::*;

#[inline]
fn build_obj<const N: usize>(entries: [(&'static str, Variant); N]) -> Variant {
    Variant::Object(
        entries
            .into_iter()
            .map(|(key, value)| (Arc::<str>::from(key), value))
            .collect(),
    )
}

#[inline]
fn obj_field<T: DeriveVariant>(obj: &BTreeMap<Arc<str>, Variant>, key: &str) -> Option<T> {
    T::from_variant(obj.get(key)?)
}

/// Absent key decodes as `None`; a present key must parse (including `Null`).
#[inline]
fn obj_opt_field<T: DeriveVariant>(
    obj: &BTreeMap<Arc<str>, Variant>,
    key: &str,
) -> Option<Option<T>> {
    match obj.get(key) {
        None => Some(None),
        Some(value) => Option::<T>::from_variant(value),
    }
}

macro_rules! impl_variant_str_enum {
    ($ty:ty { $($variant:path => $tag:literal),+ $(,)? }) => {
        impl DeriveVariant for $ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                let raw = value.as_str()?.trim();
                $(if raw.eq_ignore_ascii_case($tag) {
                    return Some($variant);
                })+
                None
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(match self {
                    $($variant => $tag,)+
                })
            }
        }
    };
}

impl_variant_str_enum!(HdrMode {
    HdrMode::Off => "off",
    HdrMode::Auto => "auto",
    HdrMode::On => "on",
});

impl_variant_str_enum!(HdrColorSpace {
    HdrColorSpace::SdrSrgb => "sdr_srgb",
    HdrColorSpace::ExtendedSrgbLinear => "extended_srgb_linear",
    HdrColorSpace::ExtendedSrgb => "extended_srgb",
    HdrColorSpace::Bt2100Pq => "bt2100_pq",
});

impl_variant_str_enum!(HdrFallback {
    HdrFallback::Disabled => "disabled",
    HdrFallback::SurfaceUnsupported => "surface_unsupported",
    HdrFallback::DisplayUnavailable => "display_unavailable",
});

impl_variant_str_enum!(IKTargetSolver {
    IKTargetSolver::FABRIK => "fabrik",
    IKTargetSolver::CCD => "ccd",
});

impl DeriveVariant for TextureFilterMode {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        Self::parse(value.as_str()?)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_str())
    }
}

impl DeriveVariant for SignedUnit {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32().map(Self::new).or_else(|| {
            value
                .as_number()
                .and_then(|value| value.as_u64_lossy())
                .and_then(|value| u8::try_from(value).ok())
                .map(Self::from_u8)
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.to_f32())
    }
}

// Rides the Vector2 member: each packed u8 lane survives the f32 round trip
// exactly (`to_f32` is injective over 256 values and `new` rounds back).
impl DeriveVariant for SignedUnitVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec2() {
            return Some(Self::from(v));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        Some(Self::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_vector2())
    }
}

// Integer = raw bits; array = 1-based layer list.
impl DeriveVariant for BitMask {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(bits) = variant_to_u32(value) {
            return Some(Self::from_bits(bits));
        }
        let items = value.as_array()?;
        let mut layers = Vec::with_capacity(items.len());
        for item in items {
            layers.push(variant_to_u32(item)?);
        }
        Self::try_from_layers(layers)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.bits())
    }
}

impl DeriveVariant for CollisionPolicy {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            layers: obj_field(obj, "layers")?,
            mask: obj_field(obj, "mask")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("layers", self.layers.to_variant()),
            ("mask", self.mask.to_variant()),
        ])
    }
}

impl DeriveVariant for NodeModulate {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            modulate: obj_field(obj, "modulate")?,
            self_modulate: obj_field(obj, "self_modulate")?,
            children_modulate: obj_field(obj, "children_modulate")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("modulate", Variant::from(self.modulate)),
            ("self_modulate", Variant::from(self.self_modulate)),
            ("children_modulate", Variant::from(self.children_modulate)),
        ])
    }
}

impl DeriveVariant for AudioMaterial {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            absorption: obj_field(obj, "absorption")?,
            reflection: obj_field(obj, "reflection")?,
            transmission: obj_field(obj, "transmission")?,
            diffusion: obj_field(obj, "diffusion")?,
            low_pass_strength: obj_field(obj, "low_pass_strength")?,
            thickness_multiplier: obj_field(obj, "thickness_multiplier")?,
            audio_mask: obj_field(obj, "audio_mask")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("absorption", Variant::from(self.absorption)),
            ("reflection", Variant::from(self.reflection)),
            ("transmission", Variant::from(self.transmission)),
            ("diffusion", Variant::from(self.diffusion)),
            ("low_pass_strength", Variant::from(self.low_pass_strength)),
            (
                "thickness_multiplier",
                Variant::from(self.thickness_multiplier),
            ),
            ("audio_mask", self.audio_mask.to_variant()),
        ])
    }
}

impl DeriveVariant for AudioDiffusion {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            damping: obj_field(obj, "damping")?,
            compression: obj_field(obj, "compression")?,
            hardness: obj_field(obj, "hardness")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("damping", Variant::from(self.damping)),
            ("compression", Variant::from(self.compression)),
            ("hardness", Variant::from(self.hardness)),
        ])
    }
}

impl DeriveVariant for AudioInteraction {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            material: obj_field(obj, "material")?,
            diffusion: obj_field(obj, "diffusion")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("material", self.material.to_variant()),
            ("diffusion", self.diffusion.to_variant()),
        ])
    }
}

impl DeriveVariant for AudioEffect {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            reverb_send: obj_field(obj, "reverb_send")?,
            echo: obj_field(obj, "echo")?,
            dampening: obj_field(obj, "dampening")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("reverb_send", Variant::from(self.reverb_send)),
            ("echo", Variant::from(self.echo)),
            ("dampening", Variant::from(self.dampening)),
        ])
    }
}

impl DeriveVariant for AudioListenerOptions {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            audio_mask: obj_field(obj, "audio_mask")?,
            effects: obj_field(obj, "effects")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("audio_mask", self.audio_mask.to_variant()),
            ("effects", self.effects.to_variant()),
        ])
    }
}

// Untagged: value shape picks the member. Floats stay F32, ints become I32,
// vec sizes disambiguate the rest.
impl DeriveVariant for ConstParamValue {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        match value {
            Variant::Bool(v) => Some(Self::Bool(*v)),
            Variant::Number(number) => match number {
                Number::F32(v) => Some(Self::F32(*v)),
                Number::F64(v) => Some(Self::F32(*v as f32)),
                _ => number
                    .as_i64_lossy()
                    .and_then(|v| i32::try_from(v).ok())
                    .map(Self::I32),
            },
            _ => {
                if let Some(v) = value.as_vec2() {
                    return Some(Self::Vec2([v.x, v.y]));
                }
                if let Some(v) = value.as_vec3() {
                    return Some(Self::Vec3([v.x, v.y, v.z]));
                }
                if let Some(v) = value.as_vec4() {
                    return Some(Self::Vec4([v.x, v.y, v.z, v.w]));
                }
                match value.as_array()? {
                    [x, y] => Some(Self::Vec2([x.as_f32()?, y.as_f32()?])),
                    [x, y, z] => Some(Self::Vec3([x.as_f32()?, y.as_f32()?, z.as_f32()?])),
                    [x, y, z, w] => Some(Self::Vec4([
                        x.as_f32()?,
                        y.as_f32()?,
                        z.as_f32()?,
                        w.as_f32()?,
                    ])),
                    _ => None,
                }
            }
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        match self {
            Self::F32(v) => Variant::from(*v),
            Self::I32(v) => Variant::from(*v),
            Self::Bool(v) => Variant::from(*v),
            Self::Vec2(v) => Variant::from(Vector2::new(v[0], v[1])),
            Self::Vec3(v) => Variant::from(Vector3::new(v[0], v[1], v[2])),
            Self::Vec4(v) => Variant::from(Vector4::new(v[0], v[1], v[2], v[3])),
        }
    }
}

impl DeriveVariant for HdrStatus {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            requested: obj_field(obj, "requested")?,
            supported: obj_field(obj, "supported")?,
            active: obj_field(obj, "active")?,
            scene_hdr: obj_field(obj, "scene_hdr")?,
            color_space: obj_field(obj, "color_space")?,
            headroom: obj_field(obj, "headroom")?,
            peak_nits: obj_opt_field(obj, "peak_nits")?,
            fallback: obj_opt_field(obj, "fallback")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("requested", self.requested.to_variant()),
            ("supported", Variant::from(self.supported)),
            ("active", Variant::from(self.active)),
            ("scene_hdr", Variant::from(self.scene_hdr)),
            ("color_space", self.color_space.to_variant()),
            ("headroom", Variant::from(self.headroom)),
            ("peak_nits", self.peak_nits.to_variant()),
            ("fallback", self.fallback.to_variant()),
        ])
    }
}

impl DeriveVariant for IKTargetParams {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            skeleton: obj_field(obj, "skeleton")?,
            bone_index: obj_field(obj, "bone_index")?,
            chain_length: obj_field(obj, "chain_length")?,
            iterations: obj_field(obj, "iterations")?,
            tolerance: obj_field(obj, "tolerance")?,
            weight: obj_field(obj, "weight")?,
            match_rotation: obj_field(obj, "match_rotation")?,
            solver: obj_field(obj, "solver")?,
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        build_obj([
            ("skeleton", Variant::from(self.skeleton)),
            ("bone_index", Variant::from(self.bone_index)),
            ("chain_length", Variant::from(self.chain_length)),
            ("iterations", Variant::from(self.iterations)),
            ("tolerance", Variant::from(self.tolerance)),
            ("weight", Variant::from(self.weight)),
            ("match_rotation", Variant::from(self.match_rotation)),
            ("solver", self.solver.to_variant()),
        ])
    }
}

fn parse_draw_shape(
    value: &Variant,
    resolver: Option<&mut dyn SceneVariantResolver>,
) -> Option<DrawShape2D> {
    let obj = value.as_object()?;
    match obj.get("type")?.as_str()? {
        "circle" => Some(DrawShape2D::Circle {
            radius: obj_field(obj, "radius")?,
            color: obj_field(obj, "color")?,
            filled: obj_field(obj, "filled")?,
            thickness: obj_field(obj, "thickness")?,
        }),
        "rect" => Some(DrawShape2D::Rect {
            size: obj_field(obj, "size")?,
            color: obj_field(obj, "color")?,
            filled: obj_field(obj, "filled")?,
            thickness: obj_field(obj, "thickness")?,
        }),
        "line" => Some(DrawShape2D::Line {
            end: obj_field(obj, "end")?,
            color: obj_field(obj, "color")?,
            thickness: obj_field(obj, "thickness")?,
        }),
        "polyline" => Some(DrawShape2D::Polyline {
            points: obj_field::<Vec<Vector2>>(obj, "points")?.into(),
            color: obj_field(obj, "color")?,
            thickness: obj_field(obj, "thickness")?,
            closed: obj_field(obj, "closed")?,
        }),
        "path" => Some(DrawShape2D::Path {
            points: obj_field::<Vec<Vector2>>(obj, "points")?.into(),
            color: obj_field(obj, "color")?,
            thickness: obj_field(obj, "thickness")?,
        }),
        "sprite" => {
            let texture_value = obj.get("texture")?;
            let texture = match resolver {
                Some(resolver) => TextureID::from_scene_variant(texture_value, resolver)?,
                None => TextureID::from_variant(texture_value)?,
            };
            Some(DrawShape2D::Sprite {
                texture,
                size: obj_field(obj, "size")?,
                tint: obj_field(obj, "tint")?,
                texture_region: obj_opt_field(obj, "texture_region")?,
            })
        }
        _ => None,
    }
}

impl DeriveVariant for DrawShape2D {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        parse_draw_shape(value, None)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        parse_draw_shape(value, Some(resolver))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let points_variant = |points: &Arc<[Vector2]>| {
            Variant::Array(points.iter().map(|p| Variant::from(*p)).collect())
        };
        match self {
            Self::Circle {
                radius,
                color,
                filled,
                thickness,
            } => build_obj([
                ("type", Variant::from("circle")),
                ("radius", Variant::from(*radius)),
                ("color", Variant::from(*color)),
                ("filled", Variant::from(*filled)),
                ("thickness", Variant::from(*thickness)),
            ]),
            Self::Rect {
                size,
                color,
                filled,
                thickness,
            } => build_obj([
                ("type", Variant::from("rect")),
                ("size", Variant::from(*size)),
                ("color", Variant::from(*color)),
                ("filled", Variant::from(*filled)),
                ("thickness", Variant::from(*thickness)),
            ]),
            Self::Line {
                end,
                color,
                thickness,
            } => build_obj([
                ("type", Variant::from("line")),
                ("end", Variant::from(*end)),
                ("color", Variant::from(*color)),
                ("thickness", Variant::from(*thickness)),
            ]),
            Self::Polyline {
                points,
                color,
                thickness,
                closed,
            } => build_obj([
                ("type", Variant::from("polyline")),
                ("points", points_variant(points)),
                ("color", Variant::from(*color)),
                ("thickness", Variant::from(*thickness)),
                ("closed", Variant::from(*closed)),
            ]),
            Self::Path {
                points,
                color,
                thickness,
            } => build_obj([
                ("type", Variant::from("path")),
                ("points", points_variant(points)),
                ("color", Variant::from(*color)),
                ("thickness", Variant::from(*thickness)),
            ]),
            Self::Sprite {
                texture,
                size,
                tint,
                texture_region,
            } => build_obj([
                ("type", Variant::from("sprite")),
                ("texture", Variant::from(*texture)),
                ("size", Variant::from(*size)),
                ("tint", Variant::from(*tint)),
                ("texture_region", texture_region.to_variant()),
            ]),
        }
    }
}
