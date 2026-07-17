use super::*;

// -------------------- JSON conversion --------------------

impl Variant {
    pub fn from_json_value(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => Variant::Null,
            JsonValue::Bool(v) => Variant::Bool(v),
            JsonValue::Number(v) => {
                if let Some(i) = v.as_i64() {
                    Variant::from(i)
                } else if let Some(u) = v.as_u64() {
                    Variant::from(u)
                } else if let Some(f) = v.as_f64() {
                    Variant::from(f)
                } else {
                    Variant::Null
                }
            }
            JsonValue::String(v) => Variant::from(v),
            JsonValue::Array(values) => {
                Variant::Array(values.into_iter().map(Variant::from_json_value).collect())
            }
            JsonValue::Object(object) => Variant::Object(
                object
                    .into_iter()
                    .map(|(k, v)| (Arc::<str>::from(k), Variant::from_json_value(v)))
                    .collect::<BTreeMap<Arc<str>, Variant>>(),
            ),
        }
    }

    pub fn to_json_value(&self) -> JsonValue {
        match self {
            Variant::Null => JsonValue::Null,
            Variant::Bool(v) => JsonValue::Bool(*v),
            Variant::Number(v) => number_to_json_value(*v),
            Variant::String(v) => JsonValue::String(v.as_ref().to_string()),
            Variant::Bytes(v) => JsonValue::Array(
                v.iter()
                    .map(|b| JsonValue::Number(JsonNumber::from(*b)))
                    .collect(),
            ),
            Variant::ID(v) => JsonValue::Number(JsonNumber::from(v.as_u64())),
            Variant::EngineStruct(EngineStruct::Vector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Vector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Vector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                map.insert("w".to_string(), float_to_json(v.w as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                map.insert("w".to_string(), JsonValue::Number(JsonNumber::from(v.w)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                map.insert("w".to_string(), JsonValue::Number(JsonNumber::from(v.w)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                map.insert("z".to_string(), float_to_json(v.z.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                map.insert("z".to_string(), float_to_json(v.z.to_f32() as f64));
                map.insert("w".to_string(), float_to_json(v.w.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Matrix2(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Matrix3(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Matrix4(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Transform2D(v)) => {
                let mut position = JsonMap::new();
                position.insert("x".to_string(), float_to_json(v.position.x as f64));
                position.insert("y".to_string(), float_to_json(v.position.y as f64));

                let mut scale = JsonMap::new();
                scale.insert("x".to_string(), float_to_json(v.scale.x as f64));
                scale.insert("y".to_string(), float_to_json(v.scale.y as f64));

                let mut map = JsonMap::new();
                map.insert("position".to_string(), JsonValue::Object(position));
                map.insert("scale".to_string(), JsonValue::Object(scale));
                map.insert("rotation".to_string(), float_to_json(v.rotation as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Transform3D(v)) => {
                let mut position = JsonMap::new();
                position.insert("x".to_string(), float_to_json(v.position.x as f64));
                position.insert("y".to_string(), float_to_json(v.position.y as f64));
                position.insert("z".to_string(), float_to_json(v.position.z as f64));

                let mut scale = JsonMap::new();
                scale.insert("x".to_string(), float_to_json(v.scale.x as f64));
                scale.insert("y".to_string(), float_to_json(v.scale.y as f64));
                scale.insert("z".to_string(), float_to_json(v.scale.z as f64));

                let mut rotation = JsonMap::new();
                rotation.insert("x".to_string(), float_to_json(v.rotation.x as f64));
                rotation.insert("y".to_string(), float_to_json(v.rotation.y as f64));
                rotation.insert("z".to_string(), float_to_json(v.rotation.z as f64));
                rotation.insert("w".to_string(), float_to_json(v.rotation.w as f64));

                let mut map = JsonMap::new();
                map.insert("position".to_string(), JsonValue::Object(position));
                map.insert("scale".to_string(), JsonValue::Object(scale));
                map.insert("rotation".to_string(), JsonValue::Object(rotation));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Quaternion(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                map.insert("w".to_string(), float_to_json(v.w as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => {
                JsonValue::Array(v.entries().iter().map(post_process_entry_to_json).collect())
            }
            Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v)) => {
                let mut map = JsonMap::new();
                let color_blind = match v.color_blind {
                    Some(setting) => {
                        let mut cb = JsonMap::new();
                        cb.insert(
                            "filter".to_string(),
                            JsonValue::String(
                                color_blind_filter_to_str(setting.filter).to_string(),
                            ),
                        );
                        cb.insert(
                            "strength".to_string(),
                            float_to_json(setting.strength as f64),
                        );
                        JsonValue::Object(cb)
                    }
                    None => JsonValue::Null,
                };
                map.insert("color_blind".to_string(), color_blind);
                JsonValue::Object(map)
            }
            Variant::Array(v) => JsonValue::Array(v.iter().map(Variant::to_json_value).collect()),
            Variant::Object(v) => JsonValue::Object(
                v.iter()
                    .map(|(k, v)| (k.as_ref().to_string(), v.to_json_value()))
                    .collect::<JsonMap<String, JsonValue>>(),
            ),
        }
    }
}

pub(super) fn number_to_json_value(number: Number) -> JsonValue {
    match number {
        Number::I8(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I16(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I32(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I64(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I128(v) => match i64::try_from(v.get()) {
            Ok(v) => JsonValue::Number(JsonNumber::from(v)),
            Err(_) => JsonValue::String(v.get().to_string()),
        },
        Number::U8(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U16(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U32(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U64(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U128(v) => match u64::try_from(v.get()) {
            Ok(v) => JsonValue::Number(JsonNumber::from(v)),
            Err(_) => JsonValue::String(v.get().to_string()),
        },
        Number::F32(v) => float_to_json(v as f64),
        Number::F64(v) => float_to_json(v),
    }
}

pub(super) fn float_to_json(value: f64) -> JsonValue {
    match JsonNumber::from_f64(value) {
        Some(v) => JsonValue::Number(v),
        None => JsonValue::Null,
    }
}

pub(super) fn color_blind_filter_to_str(filter: ColorBlindFilter) -> &'static str {
    match filter {
        ColorBlindFilter::Protan => "protan",
        ColorBlindFilter::Deuteran => "deuteran",
        ColorBlindFilter::Tritan => "tritan",
        ColorBlindFilter::Achroma => "achroma",
    }
}

pub(super) fn custom_post_param_value_to_json(value: &CustomPostParamValue) -> JsonValue {
    match value {
        CustomPostParamValue::F32(v) => float_to_json(*v as f64),
        CustomPostParamValue::I32(v) => JsonValue::Number(JsonNumber::from(*v)),
        CustomPostParamValue::Bool(v) => JsonValue::Bool(*v),
        CustomPostParamValue::Vec2(v) => {
            JsonValue::Array(vec![float_to_json(v[0] as f64), float_to_json(v[1] as f64)])
        }
        CustomPostParamValue::Vec3(v) => JsonValue::Array(vec![
            float_to_json(v[0] as f64),
            float_to_json(v[1] as f64),
            float_to_json(v[2] as f64),
        ]),
        CustomPostParamValue::Vec4(v) => JsonValue::Array(vec![
            float_to_json(v[0] as f64),
            float_to_json(v[1] as f64),
            float_to_json(v[2] as f64),
            float_to_json(v[3] as f64),
        ]),
    }
}

pub(super) fn post_process_entry_to_json(entry: &PostProcessEntry) -> JsonValue {
    let mut value = post_process_effect_to_json(&entry.effect);
    if let JsonValue::Object(map) = &mut value {
        match &entry.name {
            Some(name) => {
                map.insert("name".to_string(), JsonValue::String(name.to_string()));
            }
            None => {
                map.insert("name".to_string(), JsonValue::Null);
            }
        }
    }
    value
}

pub(super) fn post_process_effect_to_json(effect: &PostProcessEffect) -> JsonValue {
    let mut map = JsonMap::new();
    match effect {
        PostProcessEffect::Blur { strength } => {
            map.insert("type".to_string(), JsonValue::String("blur".to_string()));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Pixelate { size } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("pixelate".to_string()),
            );
            map.insert("size".to_string(), float_to_json(*size as f64));
        }
        PostProcessEffect::Warp { waves, strength } => {
            map.insert("type".to_string(), JsonValue::String("warp".to_string()));
            map.insert("waves".to_string(), float_to_json(*waves as f64));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Vignette {
            strength,
            radius,
            softness,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("vignette".to_string()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("radius".to_string(), float_to_json(*radius as f64));
            map.insert("softness".to_string(), float_to_json(*softness as f64));
        }
        PostProcessEffect::Crt {
            scanline_strength,
            curvature,
            chromatic,
            vignette,
        } => {
            map.insert("type".to_string(), JsonValue::String("crt".to_string()));
            map.insert(
                "scanline_strength".to_string(),
                float_to_json(*scanline_strength as f64),
            );
            map.insert("curvature".to_string(), float_to_json(*curvature as f64));
            map.insert("chromatic".to_string(), float_to_json(*chromatic as f64));
            map.insert("vignette".to_string(), float_to_json(*vignette as f64));
        }
        PostProcessEffect::ColorFilter { color, strength } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("color_filter".to_string()),
            );
            map.insert(
                "color".to_string(),
                JsonValue::Array(color.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::ReverseFilter {
            color,
            strength,
            softness,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("reverse_filter".to_string()),
            );
            map.insert(
                "color".to_string(),
                JsonValue::Array(color.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("softness".to_string(), float_to_json(*softness as f64));
        }
        PostProcessEffect::Bloom {
            strength,
            threshold,
            radius,
        } => {
            map.insert("type".to_string(), JsonValue::String("bloom".to_string()));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("threshold".to_string(), float_to_json(*threshold as f64));
            map.insert("radius".to_string(), float_to_json(*radius as f64));
        }
        PostProcessEffect::Exposure {
            exposure,
            auto_exposure,
            min_exposure,
            max_exposure,
            speed_up,
            speed_down,
            target_luminance,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("exposure".to_string()),
            );
            map.insert("exposure".to_string(), float_to_json(*exposure as f64));
            map.insert("auto_exposure".to_string(), JsonValue::Bool(*auto_exposure));
            map.insert(
                "min_exposure".to_string(),
                float_to_json(*min_exposure as f64),
            );
            map.insert(
                "max_exposure".to_string(),
                float_to_json(*max_exposure as f64),
            );
            map.insert("speed_up".to_string(), float_to_json(*speed_up as f64));
            map.insert("speed_down".to_string(), float_to_json(*speed_down as f64));
            map.insert(
                "target_luminance".to_string(),
                float_to_json(*target_luminance as f64),
            );
        }
        PostProcessEffect::Saturate { amount } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("saturate".to_string()),
            );
            map.insert("amount".to_string(), float_to_json(*amount as f64));
        }
        PostProcessEffect::BlackWhite { amount } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("black_white".to_string()),
            );
            map.insert("amount".to_string(), float_to_json(*amount as f64));
        }
        PostProcessEffect::ColorGrade {
            exposure,
            contrast,
            brightness,
            saturation,
            gamma,
            temperature,
            tint,
            hue_shift,
            vibrance,
            lift,
            gain,
            offset,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("color_grade".to_string()),
            );
            map.insert("exposure".to_string(), float_to_json(*exposure as f64));
            map.insert("contrast".to_string(), float_to_json(*contrast as f64));
            map.insert("brightness".to_string(), float_to_json(*brightness as f64));
            map.insert("saturation".to_string(), float_to_json(*saturation as f64));
            map.insert("gamma".to_string(), float_to_json(*gamma as f64));
            map.insert(
                "temperature".to_string(),
                float_to_json(*temperature as f64),
            );
            map.insert("tint".to_string(), float_to_json(*tint as f64));
            map.insert("hue_shift".to_string(), float_to_json(*hue_shift as f64));
            map.insert("vibrance".to_string(), float_to_json(*vibrance as f64));
            map.insert(
                "lift".to_string(),
                JsonValue::Array(lift.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert(
                "gain".to_string(),
                JsonValue::Array(gain.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert(
                "offset".to_string(),
                JsonValue::Array(offset.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
        }
        PostProcessEffect::Lut2D {
            texture_path,
            size,
            strength,
        } => {
            map.insert("type".to_string(), JsonValue::String("lut2d".to_string()));
            map.insert(
                "texture_path".to_string(),
                JsonValue::String(texture_path.to_string()),
            );
            map.insert(
                "size".to_string(),
                JsonValue::Number(JsonNumber::from(*size)),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Lut3D {
            texture_path,
            size,
            strength,
        } => {
            map.insert("type".to_string(), JsonValue::String("lut3d".to_string()));
            map.insert(
                "texture_path".to_string(),
                JsonValue::String(texture_path.to_string()),
            );
            map.insert(
                "size".to_string(),
                JsonValue::Number(JsonNumber::from(*size)),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Custom {
            shader_path,
            params,
        } => {
            map.insert("type".to_string(), JsonValue::String("custom".to_string()));
            map.insert(
                "shader_path".to_string(),
                JsonValue::String(shader_path.to_string()),
            );
            let json_params = params
                .iter()
                .map(|p| {
                    let mut pmap = JsonMap::new();
                    match &p.name {
                        Some(name) => {
                            pmap.insert("name".to_string(), JsonValue::String(name.to_string()));
                        }
                        None => {
                            pmap.insert("name".to_string(), JsonValue::Null);
                        }
                    }
                    pmap.insert(
                        "value".to_string(),
                        custom_post_param_value_to_json(&p.value),
                    );
                    JsonValue::Object(pmap)
                })
                .collect();
            map.insert("params".to_string(), JsonValue::Array(json_params));
        }
    }
    JsonValue::Object(map)
}

pub(super) fn parse_matrix_rows<const N: usize>(value: &Variant) -> Option<[[f32; N]; N]> {
    if let Variant::Object(obj) = value
        && let Some(rows) = obj.get("rows")
    {
        return parse_matrix_rows::<N>(rows);
    }

    let values = value.as_array()?;
    let mut rows = [[0.0; N]; N];
    if values.len() == N {
        for row in 0..N {
            let cols = values[row].as_array()?;
            if cols.len() != N {
                return None;
            }
            for col in 0..N {
                rows[row][col] = cols[col].as_f32()?;
            }
        }
        return Some(rows);
    }

    if values.len() == N * N {
        for row in 0..N {
            for col in 0..N {
                rows[row][col] = values[row * N + col].as_f32()?;
            }
        }
        return Some(rows);
    }

    None
}

pub(super) fn parse_matrix_rows_generic<const ROWS: usize, const COLS: usize, T>(
    value: &Variant,
) -> Option<Matrix<ROWS, COLS, T>>
where
    T: VariantMatrixCell,
{
    if let Variant::Object(obj) = value
        && let Some(rows) = obj.get("rows")
    {
        return parse_matrix_rows_generic(rows);
    }

    // `to_variant()` on a square 2x2/3x3/4x4 matrix takes the fast path and
    // produces a `Variant::EngineStruct(Matrix2/3/4(..))`, not a plain
    // array. Recognize that shape directly (no `serde_json` round trip
    // needed) by rebuilding it as row arrays and re-dispatching.
    if let Variant::EngineStruct(engine_struct) = value {
        let rows: Option<[[f32; 4]; 4]> = match (engine_struct, ROWS, COLS) {
            (EngineStruct::Matrix2(m), 2, 2) => {
                let r = m.to_rows();
                Some([
                    [r[0][0], r[0][1], 0.0, 0.0],
                    [r[1][0], r[1][1], 0.0, 0.0],
                    [0.0; 4],
                    [0.0; 4],
                ])
            }
            (EngineStruct::Matrix3(m), 3, 3) => {
                let r = m.to_rows();
                Some([
                    [r[0][0], r[0][1], r[0][2], 0.0],
                    [r[1][0], r[1][1], r[1][2], 0.0],
                    [r[2][0], r[2][1], r[2][2], 0.0],
                    [0.0; 4],
                ])
            }
            (EngineStruct::Matrix4(m), 4, 4) => Some(m.to_rows()),
            _ => None,
        };
        if let Some(rows) = rows {
            let array = Variant::Array(
                rows.iter()
                    .take(ROWS)
                    .map(|row| {
                        Variant::Array(row.iter().take(COLS).copied().map(Variant::from).collect())
                    })
                    .collect(),
            );
            return parse_matrix_rows_generic(&array);
        }
        return None;
    }

    let values = value.as_array()?;
    let mut rows = Vec::with_capacity(ROWS);
    if values.len() == ROWS {
        for row in values {
            let cols = row.as_array()?;
            if cols.len() != COLS {
                return None;
            }
            let row = cols
                .iter()
                .map(T::from_matrix_cell_variant)
                .collect::<Option<Vec<_>>>()?
                .try_into()
                .ok()?;
            rows.push(row);
        }
    } else if values.len() == ROWS * COLS {
        for row in 0..ROWS {
            let start = row * COLS;
            let row = values[start..start + COLS]
                .iter()
                .map(T::from_matrix_cell_variant)
                .collect::<Option<Vec<_>>>()?
                .try_into()
                .ok()?;
            rows.push(row);
        }
    } else {
        return None;
    }

    Some(Matrix::new(rows.try_into().ok()?))
}

pub(super) fn matrix_to_fast_variant<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Option<Variant>
where
    T: VariantMatrixCell,
{
    if ROWS != COLS {
        return None;
    }
    let values = matrix_to_f32_values(matrix)?;
    match ROWS {
        2 => Some(Variant::from(Matrix2::from_rows([
            [values[0], values[1]],
            [values[2], values[3]],
        ]))),
        3 => Some(Variant::from(Matrix3::from_rows([
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ]))),
        4 => Some(Variant::from(Matrix4::from_rows([
            [values[0], values[1], values[2], values[3]],
            [values[4], values[5], values[6], values[7]],
            [values[8], values[9], values[10], values[11]],
            [values[12], values[13], values[14], values[15]],
        ]))),
        _ => None,
    }
}

pub(super) fn matrix_to_f32_values<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Option<Vec<f32>>
where
    T: VariantMatrixCell,
{
    let mut out = Vec::with_capacity(ROWS * COLS);
    for row in matrix.rows() {
        for cell in row {
            out.push(cell.as_matrix_cell_f32()?);
        }
    }
    Some(out)
}

pub(super) fn matrix_to_variant_array<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Variant
where
    T: VariantMatrixCell,
{
    Variant::Array(
        matrix
            .rows()
            .iter()
            .map(|row| {
                Variant::Array(
                    row.iter()
                        .map(VariantMatrixCell::to_matrix_cell_variant)
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

pub(super) fn matrix_rows_to_json<const N: usize>(rows: [[f32; N]; N]) -> JsonValue {
    JsonValue::Array(
        rows.into_iter()
            .map(|row| JsonValue::Array(row.into_iter().map(|v| float_to_json(v as f64)).collect()))
            .collect(),
    )
}

pub(super) fn variant_to_u32(value: &Variant) -> Option<u32> {
    match value.as_number()? {
        Number::I8(v) => u32::try_from(v).ok(),
        Number::I16(v) => u32::try_from(v).ok(),
        Number::I32(v) => u32::try_from(v).ok(),
        Number::I64(v) => u32::try_from(v).ok(),
        Number::I128(v) => u32::try_from(v.get()).ok(),
        Number::U8(v) => Some(v as u32),
        Number::U16(v) => Some(v as u32),
        Number::U32(v) => Some(v),
        Number::U64(v) => u32::try_from(v).ok(),
        Number::U128(v) => u32::try_from(v.get()).ok(),
        Number::F32(_) | Number::F64(_) => None,
    }
}
