fn as_post_processing(value: &SceneValue) -> Option<PostProcessSet> {
    match value {
        SceneValue::Array(items) => {
            let mut effects = Vec::new();
            let mut names = Vec::new();
            for item in items.as_ref() {
                let (name, effect) = post_effect_from(item)?;
                effects.push(effect);
                names.push(name);
            }
            Some(PostProcessSet::from_pairs(effects, names))
        }
        SceneValue::Object(entries) => {
            let all_indexed = entries
                .iter()
                .all(|(k, _)| parse_param_key_index(k).is_some());
            if all_indexed {
                let mut indexed =
                    Vec::<(usize, Option<Cow<'static, str>>, PostProcessEffect)>::new();
                for (k, v) in entries.as_ref() {
                    let idx = parse_param_key_index(k)?;
                    let (name, effect) = post_effect_from(v)?;
                    indexed.push((idx, name, effect));
                }
                if indexed.is_empty() {
                    return Some(PostProcessSet::new());
                }
                indexed.sort_unstable_by_key(|(i, _, _)| *i);
                let mut effects = Vec::with_capacity(indexed.len());
                let mut names = Vec::with_capacity(indexed.len());
                for (_, name, effect) in indexed {
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            } else {
                let mut effects = Vec::with_capacity(entries.len());
                let mut names = Vec::with_capacity(entries.len());
                for (k, v) in entries.as_ref() {
                    let (mut name, effect) = post_effect_from(v)?;
                    if name.is_none() {
                        name = Some(Cow::Owned(k.to_string()));
                    }
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            }
        }
        _ => None,
    }
}

fn post_effect_from(
    value: &SceneValue,
) -> Option<(Option<Cow<'static, str>>, PostProcessEffect)> {
    let SceneValue::Object(entries) = value else {
        return None;
    };
    let mut name: Option<Cow<'static, str>> = None;
    let mut ty: Option<String> = None;
    let mut strength: Option<f32> = None;
    let mut size: Option<f32> = None;
    let mut waves: Option<f32> = None;
    let mut radius: Option<f32> = None;
    let mut softness: Option<f32> = None;
    let mut scanline_strength: Option<f32> = None;
    let mut curvature: Option<f32> = None;
    let mut chromatic: Option<f32> = None;
    let mut vignette: Option<f32> = None;
    let mut color: Option<[f32; 3]> = None;
    let mut threshold: Option<f32> = None;
    let mut amount: Option<f32> = None;
    let mut shader_path: Option<String> = None;
    let mut params: Option<Vec<CustomPostParam>> = None;

    for (k, v) in entries.as_ref() {
        match k.as_ref() {
            "name" | "id" | "key" => {
                if let Some(s) = as_str(v) {
                    let s = s.trim();
                    if !s.is_empty() {
                        name = Some(Cow::Owned(s.to_string()));
                    }
                }
            }
            "type" | "effect" => {
                if let Some(s) = as_str(v) {
                    ty = Some(s.trim().to_ascii_lowercase());
                }
            }
            "strength" => strength = as_f32(v),
            "size" => size = as_f32(v),
            "waves" => waves = as_f32(v),
            "radius" => radius = as_f32(v),
            "softness" | "feather" => softness = as_f32(v),
            "scanlines" | "scanline_strength" => scanline_strength = as_f32(v),
            "curvature" => curvature = as_f32(v),
            "chromatic" | "chromatic_aberration" => chromatic = as_f32(v),
            "vignette" => vignette = as_f32(v),
            "color" | "tint" => {
                if let Some(c) = as_vec3(v) {
                    color = Some([c.x, c.y, c.z]);
                }
            }
            "threshold" => threshold = as_f32(v),
            "amount" => amount = as_f32(v),
            "shader" | "shader_path" => {
                if let Some(s) = as_str(v) {
                    shader_path = Some(s.to_string());
                }
            }
            "params" => params = as_post_params(v),
            _ => {}
        }
    }

    match ty.as_deref()? {
        "blur" => Some((
            name,
            PostProcessEffect::Blur {
                strength: strength.unwrap_or(1.0),
            },
        )),
        "pixel" | "pixelate" => Some((
            name,
            PostProcessEffect::Pixelate {
                size: size.unwrap_or(1.0),
            },
        )),
        "warp" => Some((
            name,
            PostProcessEffect::Warp {
                waves: waves.unwrap_or(1.0),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "vignette" => Some((
            name,
            PostProcessEffect::Vignette {
                strength: strength.unwrap_or(0.6),
                radius: radius.unwrap_or(0.55),
                softness: softness.unwrap_or(0.25),
            },
        )),
        "crt" => Some((
            name,
            PostProcessEffect::Crt {
                scanline_strength: scanline_strength.unwrap_or(0.35),
                curvature: curvature.unwrap_or(0.15),
                chromatic: chromatic.unwrap_or(1.0),
                vignette: vignette.unwrap_or(0.25),
            },
        )),
        "colorfilter" | "color_filter" | "filter" => Some((
            name,
            PostProcessEffect::ColorFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "reversefilter" | "reverse_filter" | "reverse" => Some((
            name,
            PostProcessEffect::ReverseFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
                softness: softness.unwrap_or(0.2),
            },
        )),
        "bloom" => Some((
            name,
            PostProcessEffect::Bloom {
                strength: strength.unwrap_or(0.6),
                threshold: threshold.unwrap_or(0.7),
                radius: radius.unwrap_or(1.25),
            },
        )),
        "saturate" | "saturation" => Some((
            name,
            PostProcessEffect::Saturate {
                amount: amount.or(strength).unwrap_or(1.2),
            },
        )),
        "black_white" | "blackwhite" | "bw" | "grayscale" => Some((
            name,
            PostProcessEffect::BlackWhite {
                amount: amount.or(strength).unwrap_or(1.0),
            },
        )),
        "custom" => {
            let shader_path = shader_path?;
            let params = params.unwrap_or_default();
            Some((
                name,
                PostProcessEffect::Custom {
                    shader_path: Cow::Owned(shader_path),
                    params: Cow::Owned(params),
                },
            ))
        }
        _ => None,
    }
}

fn as_post_params(value: &SceneValue) -> Option<Vec<CustomPostParam>> {
    match value {
        SceneValue::Array(items) => {
            let mut out = Vec::new();
            for item in items.as_ref() {
                out.push(CustomPostParam::unnamed(post_param_value(
                    item,
                )?));
            }
            Some(out)
        }
        SceneValue::Object(entries) => {
            let mut indexed = Vec::<(usize, CustomPostParam)>::new();
            for (k, v) in entries.as_ref() {
                let idx = parse_param_key_index(k)?;
                let value = post_param_value(v)?;
                indexed.push((idx, CustomPostParam::unnamed(value)));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            Some(indexed.into_iter().map(|(_, v)| v).collect())
        }
        _ => None,
    }
}

fn post_param_value(value: &SceneValue) -> Option<CustomPostParamValue> {
    value.as_const_param()
}

fn parse_param_key_index(key: &str) -> Option<usize> {
    let key = key.trim();
    if let Ok(i) = key.parse::<usize>() {
        return Some(i);
    }
    if let Some(rest) = key.strip_prefix('p')
        && let Ok(i) = rest.parse::<usize>()
    {
        return Some(i);
    }
    None
}



