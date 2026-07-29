//! Off-GPU validation of game `.wgsl` files.
//!
//! Game shaders are fragments, so they only parse once the engine prelude and
//! entry points are wrapped around them. This module composes a shader exactly
//! like the renderer does, runs naga's parser + validator over the result, and
//! maps any span back to a line/column in the author's own file.

use crate::compose::{
    self, MaterialOutput, build_custom_material_shader_with_prelude, build_post_shader,
    build_sky_shader_with_passes,
};

/// Which engine hook a game `.wgsl` file implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserShaderKind {
    /// Custom 3D material: `shade_material`, optional `shade_vertex`.
    Material3D,
    /// `Sky3D` pass: `sky_shader`.
    Sky3D,
    /// Custom post-process pass: `post_process`.
    PostProcess,
}

impl UserShaderKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Material3D => "custom 3D material",
            Self::Sky3D => "Sky3D pass",
            Self::PostProcess => "post-process pass",
        }
    }

    #[must_use]
    pub const fn entry_fn(self) -> &'static str {
        match self {
            Self::Material3D => "shade_material",
            Self::Sky3D => "sky_shader",
            Self::PostProcess => "post_process",
        }
    }
}

/// One naga parse or validation failure, resolved back to the game file.
#[derive(Clone, Debug)]
pub struct ShaderDiagnostic {
    /// Which composed variant failed, e.g. `rigid mesh`.
    pub variant: &'static str,
    /// 1-based line in the game `.wgsl`, when the span lands inside it.
    pub line: Option<usize>,
    /// 1-based column in the game `.wgsl`, when the span lands inside it.
    pub column: Option<usize>,
    pub message: String,
}

impl core::fmt::Display for ShaderDiagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(column)) => {
                write!(f, "{line}:{column}: [{}] {}", self.variant, self.message)
            }
            _ => write!(f, "[{}] {}", self.variant, self.message),
        }
    }
}

/// Guess the hook a game shader implements from its entry function.
///
/// Returns `None` when the file declares none of them; such a file is never
/// loadable by the engine on its own.
#[must_use]
pub fn detect_user_shader_kind(source: &str) -> Option<UserShaderKind> {
    let stripped = crate::optimize_source(source);
    for (needle, kind) in [
        ("fn shade_material", UserShaderKind::Material3D),
        ("fn shade_vertex", UserShaderKind::Material3D),
        ("fn sky_shader", UserShaderKind::Sky3D),
        ("fn post_process", UserShaderKind::PostProcess),
    ] {
        if stripped.contains(needle) {
            return Some(kind);
        }
    }
    None
}

/// Compose `source` the way the renderer does, then parse + validate it.
///
/// Returns the diagnostics of the first composed variant that fails; an empty
/// vec means the shader is good on every variant the engine can build from it.
#[must_use]
pub fn check_user_shader(source: &str, kind: UserShaderKind) -> Vec<ShaderDiagnostic> {
    for composed in compose_variants(source, kind) {
        let diagnostics = validate_composed(&composed);
        if !diagnostics.is_empty() {
            return diagnostics;
        }
    }
    Vec::new()
}

/// Detect the kind, then check. `Err` carries the reason a check never ran.
///
/// # Errors
///
/// Returns [`ShaderCheckError::UnknownKind`] when no engine entry function is
/// declared, and [`ShaderCheckError::Invalid`] when composition fails to parse
/// or validate.
pub fn check_user_shader_source(source: &str) -> Result<UserShaderKind, ShaderCheckError> {
    let Some(kind) = detect_user_shader_kind(source) else {
        return Err(ShaderCheckError::UnknownKind);
    };
    let diagnostics = check_user_shader(source, kind);
    if diagnostics.is_empty() {
        Ok(kind)
    } else {
        Err(ShaderCheckError::Invalid { kind, diagnostics })
    }
}

/// Why [`check_user_shader_source`] did not accept a shader.
#[derive(Clone, Debug)]
pub enum ShaderCheckError {
    /// No `shade_material`, `sky_shader`, or `post_process` declared.
    UnknownKind,
    Invalid {
        kind: UserShaderKind,
        diagnostics: Vec<ShaderDiagnostic>,
    },
}

struct ComposedShader {
    variant: &'static str,
    wgsl: String,
    user_range: Option<core::ops::Range<usize>>,
}

impl ComposedShader {
    fn new(variant: &'static str, wgsl: String, user_text: &str) -> Self {
        let user_range = (!user_text.is_empty())
            .then(|| wgsl.find(user_text))
            .flatten()
            .map(|start| start..start + user_text.len());
        Self {
            variant,
            wgsl,
            user_range,
        }
    }
}

fn compose_variants(source: &str, kind: UserShaderKind) -> Vec<ComposedShader> {
    match kind {
        // A custom material can land on a rigid or a skinned mesh; both
        // preludes have to accept it. Multimesh exposes a subset of the same
        // helpers, so it needs no separate pass.
        UserShaderKind::Material3D => vec![
            ComposedShader::new(
                "rigid mesh",
                build_custom_material_shader_with_prelude(
                    compose::prelude_rigid_wgsl(),
                    source,
                    MaterialOutput::Surface,
                ),
                source,
            ),
            ComposedShader::new(
                "skinned mesh",
                build_custom_material_shader_with_prelude(
                    compose::prelude_skinned_wgsl(),
                    source,
                    MaterialOutput::Surface,
                ),
                source,
            ),
        ],
        UserShaderKind::Sky3D => {
            let renamed = compose::rename_sky_pass(source, "sky_shader_0");
            vec![ComposedShader::new(
                "sky pass",
                build_sky_shader_with_passes(&[(source.to_string(), [[0.0; 4]; 16])]),
                &renamed,
            )]
        }
        UserShaderKind::PostProcess => vec![ComposedShader::new(
            "post pass",
            build_post_shader(source),
            source,
        )],
    }
}

fn validate_composed(composed: &ComposedShader) -> Vec<ShaderDiagnostic> {
    let module = match naga::front::wgsl::parse_str(&composed.wgsl) {
        Ok(module) => module,
        Err(err) => {
            let location = err.location(&composed.wgsl);
            return vec![diagnostic(composed, location, err.to_string())];
        }
    };
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    );
    match validator.validate(&module) {
        Ok(_) => Vec::new(),
        Err(err) => {
            let location = err.location(&composed.wgsl);
            let mut message = error_chain(err.as_inner());
            if let Some((_, context)) = err.spans().next() {
                message = format!("{message} ({context})");
            }
            vec![diagnostic(composed, location, message)]
        }
    }
}

fn diagnostic(
    composed: &ComposedShader,
    location: Option<naga::SourceLocation>,
    message: String,
) -> ShaderDiagnostic {
    let user_position = location.and_then(|loc| user_position(composed, loc.offset as usize));
    match user_position {
        Some((line, column)) => ShaderDiagnostic {
            variant: composed.variant,
            line: Some(line),
            column: Some(column),
            message,
        },
        // Span sits in the engine prelude: the game file still caused it, but
        // there is no line to point at, so say where it landed instead.
        None => ShaderDiagnostic {
            variant: composed.variant,
            line: None,
            column: None,
            message: format!("{message} (reported in engine prelude, not in this file)"),
        },
    }
}

fn user_position(composed: &ComposedShader, offset: usize) -> Option<(usize, usize)> {
    let range = composed.user_range.clone()?;
    if !range.contains(&offset) {
        return None;
    }
    let before = composed.wgsl.get(range.start..offset)?;
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    Some((line, before.len() - line_start + 1))
}

fn error_chain(err: &dyn core::error::Error) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(inner) = source {
        out.push_str(": ");
        out.push_str(&inner.to_string());
        source = inner.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATERIAL: &str = r#"
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let tint = custom_f_param(in, 0u);
    return vec4<f32>(color.rgb * tint.rgb, color.a * tint.a);
}
"#;

    const SKY: &str = r#"
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    let glow = custom_f_param(in, 0u).x;
    return vec4<f32>(in.color.rgb + vec3<f32>(glow), in.color.a);
}
"#;

    const POST: &str = r#"
fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    return vec4<f32>(color.rgb * custom_params[0].x, color.a);
}
"#;

    #[test]
    fn detects_each_user_shader_kind() {
        assert_eq!(
            detect_user_shader_kind(MATERIAL),
            Some(UserShaderKind::Material3D)
        );
        assert_eq!(detect_user_shader_kind(SKY), Some(UserShaderKind::Sky3D));
        assert_eq!(
            detect_user_shader_kind(POST),
            Some(UserShaderKind::PostProcess)
        );
        assert_eq!(
            detect_user_shader_kind("fn helper() -> f32 { return 1.0; }"),
            None
        );
    }

    #[test]
    fn ignores_entry_fn_inside_comments() {
        let source = format!("// fn sky_shader(in: SkyFragment)\n{MATERIAL}");
        assert_eq!(
            detect_user_shader_kind(&source),
            Some(UserShaderKind::Material3D)
        );
    }

    #[test]
    fn accepts_valid_shaders_of_every_kind() {
        for (source, kind) in [
            (MATERIAL, UserShaderKind::Material3D),
            (SKY, UserShaderKind::Sky3D),
            (POST, UserShaderKind::PostProcess),
        ] {
            let diagnostics = check_user_shader(source, kind);
            assert!(
                diagnostics.is_empty(),
                "{}: {:?}",
                kind.label(),
                diagnostics
            );
        }
    }

    #[test]
    fn accepts_vertex_hook_and_lit_helper() {
        let source = r#"
fn shade_vertex(out_in: VertexOutput) -> VertexOutput {
    var out = out_in;
    out.world_pos.y = out.world_pos.y + sin(perro_time()) * 0.1;
    return out;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    return perro_standard(in, color, 0.5, 0.0, 1.0, vec3<f32>(0.0));
}
"#;
        let diagnostics = check_user_shader(source, UserShaderKind::Material3D);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn reports_syntax_error_at_game_file_line() {
        let source = "\nfn shade_material(in: FragmentInput) -> vec4<f32> {\n    return vec4<f32>(1.0 0.0, 0.0, 1.0);\n}\n";
        let diagnostics = check_user_shader(source, UserShaderKind::Material3D);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, Some(3));
        assert!(diagnostics[0].column.is_some());
    }

    #[test]
    fn reports_unknown_helper_at_game_file_line() {
        let source = "\nfn shade_material(in: FragmentInput) -> vec4<f32> {\n    return perro_not_a_helper(in);\n}\n";
        let diagnostics = check_user_shader(source, UserShaderKind::Material3D);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, Some(3));
    }

    #[test]
    fn reports_wrong_return_type() {
        let source =
            "fn shade_material(in: FragmentInput) -> vec3<f32> {\n    return vec3<f32>(1.0);\n}\n";
        let err = check_user_shader_source(source).expect_err("wrong return type must fail");
        let ShaderCheckError::Invalid { kind, diagnostics } = err else {
            panic!("expected invalid shader");
        };
        assert_eq!(kind, UserShaderKind::Material3D);
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn unknown_kind_is_reported_separately() {
        let err = check_user_shader_source("fn helper() -> f32 { return 1.0; }")
            .expect_err("no entry fn must fail");
        assert!(matches!(err, ShaderCheckError::UnknownKind));
    }
}
