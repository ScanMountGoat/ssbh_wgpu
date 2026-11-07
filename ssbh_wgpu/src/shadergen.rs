use std::{collections::BTreeSet, fmt::Write, str::FromStr, sync::LazyLock};

use aho_corasick::AhoCorasick;
use case::CaseExt;
use indoc::formatdoc;
use log::warn;
use smol_str::format_smolstr;
use smush_shader::{Operation, OutputExpr, Parameter, ShaderProgram, Value};
use ssbh_data::matl_data::ParamId;

use crate::uniforms::{boolean_index, float_index, vector_index};

const OUT_VAR: &str = "output";
const VERTEX_VAR_PREFIX: &str = "VS_VAR_";
const FRAGMENT_VAR_PREFIX: &str = "FS_VAR_";

static WGSL_REPLACEMENTS: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "// VERTEX_OUTPUT_FIELDS_GENERATED",
        "let ASSIGN_VS_VARS_GENERATED = 0.0;",
        "let ASSIGN_VS_OUTPUTS_GENERATED = 0.0;",
        "let ASSIGN_FS_VARS_GENERATED = 0.0;",
        "let ASSIGN_OUT_COLOR_GENERATED = 0.0;",
        "let FRAGMENT_DISCARD_GENERATED = 0.0;",
    ])
    .unwrap()
});

/// Generated WGSL model shader code for a material.
#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct ShaderWgsl {
    vertex_output_fields: String,
    vertex_assignments: String,
    vertex_outputs: String,
    fragment_assignments: String,
    fragment_outputs: String,
    fragment_discard: String,
}

impl ShaderWgsl {
    pub fn new(program: &ShaderProgram) -> Self {
        let vertex_output_fields = vertex_output_fields_wgsl(program);
        let vertex_assignments = vertex_assignments_wgsl(program);
        let vertex_outputs = vertex_outputs_wgsl(program);

        let fragment_assignments = fragment_assignments_wgsl(program);
        let fragment_outputs = fragment_outputs_wgsl(program);

        // Generate empty code if there is no discard condition.
        let fragment_discard = program
            .exprs
            .fragment_discard_condition
            .map(discard_wgsl)
            .unwrap_or_default();

        Self {
            vertex_output_fields,
            vertex_assignments,
            vertex_outputs,
            fragment_assignments,
            fragment_outputs,
            fragment_discard,
        }
    }

    pub fn create_model_shader(&self) -> String {
        let mut source = WGSL_REPLACEMENTS.replace_all(
            crate::shader::model::SOURCE,
            &[
                &self.vertex_output_fields,
                &self.vertex_assignments,
                &self.vertex_outputs,
                &self.fragment_assignments,
                &self.fragment_outputs,
                &self.fragment_discard,
            ],
        );

        // This section is only used for wgsl_to_wgpu reachability analysis and can be removed.
        if let (Some(start), Some(end)) = (
            source.find("let VS_REMOVE_BEGIN = 0.0;"),
            source.find("let VS_REMOVE_END = 0.0;"),
        ) {
            source.replace_range(start..end, "");
        }
        if let (Some(start), Some(end)) = (
            source.find("let FS_REMOVE_BEGIN = 0.0;"),
            source.find("let FS_REMOVE_END = 0.0;"),
        ) {
            source.replace_range(start..end, "");
        }

        source
    }
}

fn vertex_output_fields_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    // Declare fields and assume vec4 for all attributes.
    // TODO: Sort by location and use the location?
    let mut vars = BTreeSet::new();
    let mut location = 1; // skip the light_position for now
    for name in program.exprs.vertex_output_exprs.keys() {
        if !name.starts_with("gl_Position") {
            if let Some((name, _)) = name.split_once('.') {
                if vars.insert(name) {
                    writeln!(wgsl, "@location({location}) {name}: vec4<f32>,").unwrap();
                    location += 1;
                }
            }
        }
    }

    wgsl
}

fn vertex_assignments_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    for (i, expr) in program.exprs.vertex_exprs.iter().enumerate() {
        write!(&mut wgsl, "let {VERTEX_VAR_PREFIX}{i} = ",).unwrap();
        if write_expr(&mut wgsl, expr, VERTEX_VAR_PREFIX, true).is_none() {
            write!(wgsl, "0.0").unwrap();
        }
        writeln!(&mut wgsl, ";",).unwrap();
    }

    wgsl
}

fn fragment_assignments_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    // Map vertex outputs to fragment inputs.
    let mut vars = BTreeSet::new();
    for name in program.exprs.vertex_output_exprs.keys() {
        if !name.starts_with("gl_Position") {
            if let Some((name, _)) = name.split_once('.') {
                if vars.insert(name) {
                    writeln!(
                        wgsl,
                        "let IN_{} = in.{name};",
                        name.trim_start_matches("OUT_")
                    )
                    .unwrap();
                }
            }
        }
    }

    for (i, expr) in program.exprs.fragment_exprs.iter().enumerate() {
        write!(&mut wgsl, "let {FRAGMENT_VAR_PREFIX}{i} = ",).unwrap();
        if write_expr(&mut wgsl, expr, FRAGMENT_VAR_PREFIX, false).is_none() {
            write!(wgsl, "0.0").unwrap();
        }
        writeln!(&mut wgsl, ";",).unwrap();
    }

    wgsl
}

fn write_expr(
    wgsl: &mut String,
    expr: &OutputExpr<Operation>,
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    match expr {
        OutputExpr::Value(value) => write_value(wgsl, value, prefix, is_vertex),
        OutputExpr::Func { op, args } => write_func(wgsl, op, args, prefix),
    }
}

fn write_value(wgsl: &mut String, value: &Value, prefix: &str, is_vertex: bool) -> Option<()> {
    match value {
        Value::Int(i) => {
            write!(wgsl, "{i:?}i").unwrap();
            Some(())
        }
        Value::Uint(u) => {
            write!(wgsl, "{u:?}u").unwrap();
            Some(())
        }
        Value::Float(f) => {
            write!(wgsl, "{f:?}").unwrap();
            Some(())
        }
        Value::Bool(b) => {
            write!(wgsl, "{b:?}").unwrap();
            Some(())
        }
        Value::Parameter(p) => write_parameter(wgsl, p, prefix),
        Value::Texture(t) => write_texture(wgsl, t, prefix, is_vertex),
        Value::Attribute(a) => write_attribute(wgsl, a),
    }
}

fn write_texture(
    wgsl: &mut String,
    t: &smush_shader::Texture,
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    // Apply default values if channels are "disabled" by render settings.
    match t.name.as_str() {
        // TODO: reduce repetition
        "Texture4" => match t.channel {
            Some('x') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    0.5,
                    "render_settings.render_nor.r",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('y') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    0.5,
                    "render_settings.render_nor.g",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('z') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    0.0,
                    "render_settings.render_nor.b",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('w') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    1.0,
                    "render_settings.render_nor.a",
                    prefix,
                    is_vertex,
                )?;
            }
            _ => {
                write_texture_inner(wgsl, &t.name, &t.texcoords, prefix, is_vertex)?;
                write_channel(wgsl, t.channel);
            }
        },
        "Texture6" => match t.channel {
            Some('x') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    0.0,
                    "render_settings.render_prm.r",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('y') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    1.0,
                    "render_settings.render_prm.g",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('z') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    1.0,
                    "render_settings.render_prm.b",
                    prefix,
                    is_vertex,
                )?;
            }
            Some('w') => {
                write_texture_or_default(
                    wgsl,
                    t,
                    0.16,
                    "render_settings.render_prm.a",
                    prefix,
                    is_vertex,
                )?;
            }
            _ => {
                write_texture_inner(wgsl, &t.name, &t.texcoords, prefix, is_vertex)?;
                write_channel(wgsl, t.channel);
            }
        },
        _ => {
            write_texture_inner(wgsl, &t.name, &t.texcoords, prefix, is_vertex)?;
            write_channel(wgsl, t.channel);
        }
    }

    Some(())
}

fn write_texture_or_default(
    wgsl: &mut String,
    t: &smush_shader::Texture,
    default_value: f32,
    toggle: &str,
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    write!(wgsl, "select({default_value}, ").unwrap();
    write_texture_inner(wgsl, &t.name, &t.texcoords, prefix, is_vertex)?;
    write_channel(wgsl, t.channel);
    write!(wgsl, ", {toggle} != 0u)").unwrap();
    Some(())
}

fn write_texture_inner(
    wgsl: &mut String,
    name: &str,
    texcoords: &[usize],
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    // TODO: Support remaining textures.
    match name {
        "Texture15" => {
            // TODO: shadow map sampler?
            write_sampler_2d_or_cube(
                wgsl,
                "texture_shadow",
                "default_sampler",
                texcoords,
                prefix,
                is_vertex,
            )
        }
        "Texture16" => {
            // TODO: ink normal map for stages?
            warn!("Unsupported texture {name}");
            None
        }
        "DummyTexture" => {
            // TODO: what does this texture do?
            warn!("Unsupported texture {name}");
            None
        }
        _ => write_sampler_2d_or_cube(
            wgsl,
            &name.to_snake(),
            &name.to_snake().replace("texture", "sampler"),
            texcoords,
            prefix,
            is_vertex,
        ),
    }
}

fn write_sampler_2d_or_cube(
    wgsl: &mut String,
    name: &str,
    sampler: &str,
    texcoords: &[usize],
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    match texcoords {
        [u, v] => write_sampler_2d(wgsl, name, sampler, *u, *v, prefix, is_vertex),
        [u, v, w] => {
            // Assume 3D textures aren't used, so UVW coordinates should always be a cube map.
            if is_vertex {
                write!(
                    wgsl,
                    "textureSampleLevel({name}, {sampler}, vec3({prefix}{u}, {prefix}{v}, {prefix}{w}), 0.0)",
                )
                .unwrap();
            } else {
                write!(
                    wgsl,
                    "textureSample({name}, {sampler}, vec3({prefix}{u}, {prefix}{v}, {prefix}{w}))",
                )
                .unwrap();
            }
            Some(())
        }
        _ => None,
    }
}

fn write_sampler_2d(
    wgsl: &mut String,
    name: &str,
    sampler: &str,
    u: usize,
    v: usize,
    prefix: &str,
    is_vertex: bool,
) -> Option<()> {
    // TODO: differentiate between texture functions in the database itself.
    if is_vertex {
        write!(
            wgsl,
            "textureSampleLevel({name}, {sampler}, vec2({prefix}{u}, {prefix}{v}), 0.0)",
        )
        .unwrap();
    } else {
        write!(
            wgsl,
            "textureSample({name}, {sampler}, vec2({prefix}{u}, {prefix}{v}))",
        )
        .unwrap();
    }

    Some(())
}

fn write_attribute(wgsl: &mut String, a: &smush_shader::Attribute) -> Option<()> {
    // TODO: Handle undef during database creation?
    if a.name == "undef" {
        write!(wgsl, "0").unwrap();
        return Some(());
    }

    // TODO: Support remaining attributes.
    let name = match a.name.as_str() {
        "gl_InstanceID" => Some("0"),    // TODO: instanced rendering?
        "gl_VertexID" => Some("0"),      // TODO: vertex storage buffer indexing?
        "renormal" => Some("IN_Normal"), // TODO: use a vertex operation for renormal
        "unk_renormal" => Some("unk_renormal"),
        name => {
            if name.starts_with("IN_") {
                // Input attributes are assigned to variables previously.
                Some(name)
            } else {
                warn!("Unrecognized attribute {a}");
                None
            }
        }
    }?;

    write!(wgsl, "{name}").unwrap();
    write_channel(wgsl, a.channel);

    Some(())
}

fn write_parameter(wgsl: &mut String, p: &Parameter, prefix: &str) -> Option<()> {
    if p.field == "data" {
        warn!("Unsupported dynamic uniform field {p}");
        match p.name.as_str() {
            "EffectArea" => {
                // TODO: document EffectArea fields
                write!(wgsl, "0.0").unwrap();
                return Some(());
            }
            "adjacenciesStruct" => {
                // TODO: document adjacenciesStruct SSBO fields
                write!(wgsl, "0u").unwrap();
                return Some(());
            }
            "verticesStruct" => {
                // TODO: document verticesStruct SSBO fields
                write!(wgsl, "0u").unwrap();
                return Some(());
            }
            _ => {
                // Dynamic field lookups should be handled during database creation using queries.
                // Shader annotation can't handle cases like indexing by gl_InstanceID.
                return None;
            }
        }
    }

    // TODO: just convert case instead of matching buffer names?
    match p.name.as_str() {
        "nuPerMaterial" => {
            if let Ok(id) = ParamId::from_str(&p.field) {
                if let Some(i) = vector_index(id) {
                    write!(wgsl, "per_material.custom_vector[{i}]").unwrap();
                    write_channel(wgsl, p.channel);
                } else if let Some(i) = float_index(id) {
                    write!(wgsl, "per_material.custom_float[{i}].x").unwrap();
                } else if let Some(i) = boolean_index(id) {
                    // TODO: why is there an index for boolean params?
                    write!(wgsl, "per_material.custom_boolean[{i}].x").unwrap();
                } else {
                    warn!("Unrecognized field {}", p.field);
                    return None;
                }
            } else {
                warn!("Unrecognized field {}", p.field);
                return None;
            }
        }
        "PerObject" => {
            write_parameter_inner(wgsl, "per_object", p.field.to_snake(), p, prefix);
        }
        "ForPass" => {
            write_parameter_inner(wgsl, "for_pass", p.field.to_snake(), p, prefix);
        }
        "PerFrame" => {
            // The case conversion library doesn't always do the expected thing.
            let field = match p.field.as_str() {
                "g_IBL_ColorGain" => "g_ibl_color_gain".to_string(),
                "g_IBL_Scale" => "g_ibl_scale".to_string(),
                "dbgMaterialID" => "dbg_material_id".to_string(),
                f => f.to_snake(),
            };
            write_parameter_inner(wgsl, "per_frame", field, p, prefix);
        }
        "nuPerViewCBuffer" => {
            // The case conversion library doesn't always do the expected thing.
            let field = match p.field.as_str() {
                "inverseScreenSize2D" => "inverse_screen_size_2d".to_string(),
                "rtScaleFactor3d" => "rt_scale_factor_3d".to_string(),
                f => f.to_snake(),
            };
            write_parameter_inner(wgsl, "per_view", field, p, prefix);
        }
        "nuPerWorldCBuffer" => {
            write_parameter_inner(wgsl, "per_world", p.field.to_snake(), p, prefix);
        }
        _ => {
            warn!("Unrecognized uniform {p}");
            return None;
        }
    }
    Some(())
}

fn write_parameter_inner(
    wgsl: &mut String,
    buffer: &str,
    field: String,
    p: &Parameter,
    prefix: &str,
) {
    write!(wgsl, "{buffer}.{field}").unwrap();
    write_index(wgsl, p.index, prefix);
    write_index(wgsl, p.index2, prefix);
    write_channel(wgsl, p.channel);
}

fn write_func(wgsl: &mut String, op: &Operation, args: &[usize], prefix: &str) -> Option<()> {
    let arg0 = args.first();
    let arg1 = args.get(1);
    let arg2 = args.get(2);

    let a = prefix;
    match op {
        Operation::Unk => return None,
        Operation::Add => write!(wgsl, "{a}{} + {a}{}", arg0?, arg1?).unwrap(),
        Operation::Sub => write!(wgsl, "{a}{} - {a}{}", arg0?, arg1?).unwrap(),
        Operation::Mul => write!(wgsl, "{a}{} * {a}{}", arg0?, arg1?).unwrap(),
        Operation::Div => write!(wgsl, "{a}{} / {a}{}", arg0?, arg1?).unwrap(),
        Operation::Fma => write!(wgsl, "{a}{} * {a}{} + {a}{}", arg0?, arg1?, arg2?).unwrap(),
        Operation::Min => write!(wgsl, "min({a}{}, {a}{})", arg0?, arg1?).unwrap(),
        Operation::Max => write!(wgsl, "max({a}{}, {a}{})", arg0?, arg1?).unwrap(),
        Operation::Exp2 => write!(wgsl, "exp2({a}{})", arg0?).unwrap(),
        Operation::Clamp => {
            write!(wgsl, "clamp({a}{}, {a}{}, {a}{})", arg0?, arg1?, arg2?).unwrap()
        }
        Operation::Negate => write!(wgsl, "-({a}{})", arg0?).unwrap(),
        Operation::InverseSqrt => write!(wgsl, "inverseSqrt({a}{})", arg0?).unwrap(),
        Operation::Log2 => write!(wgsl, "log2({a}{})", arg0?).unwrap(),
        Operation::Abs => write!(wgsl, "abs({a}{})", arg0?).unwrap(),
        Operation::Sqrt => write!(wgsl, "sqrt({a}{})", arg0?).unwrap(),
        Operation::Floor => write!(wgsl, "floor({a}{})", arg0?).unwrap(),
        Operation::Trunc => write!(wgsl, "trunc({a}{})", arg0?).unwrap(),
        Operation::Sin => write!(wgsl, "sin({a}{})", arg0?).unwrap(),
        Operation::Cos => write!(wgsl, "cos({a}{})", arg0?).unwrap(),
        Operation::Select => {
            write!(wgsl, "select({a}{}, {a}{}, {a}{})", arg2?, arg1?, arg0?).unwrap()
        }
        Operation::IntBitsToFloat => write!(wgsl, "bitcast<f32>({a}{})", arg0?).unwrap(),
        Operation::UintBitsToFloat => write!(wgsl, "bitcast<f32>({a}{})", arg0?).unwrap(),
        Operation::FloatBitsToInt => write!(wgsl, "bitcast<i32>({a}{})", arg0?).unwrap(),
        Operation::FloatBitsToUint => write!(wgsl, "bitcast<u32>({a}{})", arg0?).unwrap(),
        Operation::Int => write!(wgsl, "i32({a}{})", arg0?).unwrap(),
        Operation::Uint => write!(wgsl, "u32({a}{})", arg0?).unwrap(),
        Operation::Float => write!(wgsl, "f32({a}{})", arg0?).unwrap(),
        Operation::Equal => write!(wgsl, "{a}{} == {a}{}", arg0?, arg1?).unwrap(),
        Operation::NotEqual => write!(wgsl, "{a}{} != {a}{}", arg0?, arg1?).unwrap(),
        Operation::Greater => write!(wgsl, "{a}{} > {a}{}", arg0?, arg1?).unwrap(),
        Operation::GreaterEqual => write!(wgsl, "{a}{} >= {a}{}", arg0?, arg1?).unwrap(),
        Operation::Less => write!(wgsl, "{a}{} < {a}{}", arg0?, arg1?).unwrap(),
        Operation::LessEqual => write!(wgsl, "{a}{} <= {a}{}", arg0?, arg1?).unwrap(),
        Operation::Not => write!(wgsl, "!{a}{}", arg0?).unwrap(),
        Operation::And => write!(wgsl, "{a}{} && {a}{}", arg0?, arg1?).unwrap(),
        Operation::Or => write!(wgsl, "{a}{} || {a}{}", arg0?, arg1?).unwrap(),
        Operation::LeftShift => write!(wgsl, "{a}{} << u32({a}{})", arg0?, arg1?).unwrap(),
        Operation::RightShift => write!(wgsl, "{a}{} >> u32({a}{})", arg0?, arg1?).unwrap(),
        Operation::BitAnd => write!(wgsl, "{a}{} & {a}{}", arg0?, arg1?).unwrap(),
        Operation::Pack2Float16 => {
            write!(wgsl, "pack2x16float(vec2({a}{}, {a}{}))", arg0?, arg1?).unwrap()
        }
        Operation::Unpack2Float16X => write!(wgsl, "unpack2x16float({a}{}).x", arg0?).unwrap(),
        Operation::Unpack2Float16Y => write!(wgsl, "unpack2x16float({a}{}).y", arg0?).unwrap(),
        Operation::IsNaN => write!(wgsl, "false").unwrap(), // TODO: does WGSL support this?
        Operation::VarianceShadow => {
            write!(wgsl, "GetShadow(in.light_position, {a}{})", arg0?).unwrap()
        }
    }
    Some(())
}

fn write_index(wgsl: &mut String, i: Option<usize>, prefix: &str) {
    if let Some(i) = i {
        write!(wgsl, "[{prefix}{i}]").unwrap();
    }
}

fn write_channel(wgsl: &mut String, c: Option<char>) {
    if let Some(c) = c {
        write!(wgsl, ".{c}").unwrap();
    }
}

fn vertex_outputs_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    for (name, i) in &program.exprs.vertex_output_exprs {
        if name.starts_with("gl_Position") {
            // TODO: handle position code
        } else {
            // These names already have the channel for assignment.
            writeln!(&mut wgsl, "out.{name} = {VERTEX_VAR_PREFIX}{i};").unwrap();
        }
    }

    wgsl
}

fn fragment_outputs_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    // Assume only one output for fragment shaders.
    for c in "xyzw".chars() {
        if let Some(i) = program
            .exprs
            .fragment_output_exprs
            .get(&format_smolstr!("OUT_Color.{c}"))
        {
            writeln!(&mut wgsl, "{OUT_VAR}.{c} = {FRAGMENT_VAR_PREFIX}{i};").unwrap()
        }
    }

    wgsl
}

fn discard_wgsl(condition_expr_index: usize) -> String {
    formatdoc! {"
        if {FRAGMENT_VAR_PREFIX}{condition_expr_index} {{
            discard;
        }}
    "}
}
