use std::{fmt::Write, str::FromStr, sync::LazyLock};

use aho_corasick::AhoCorasick;
use case::CaseExt;
use indoc::formatdoc;
use log::error;
use smol_str::format_smolstr;
use smush_shader::{Operation, OutputExpr, Parameter, ShaderProgram, Value};
use ssbh_data::matl_data::ParamId;

use crate::uniforms::{boolean_index, float_index, vector_index};

const OUT_VAR: &str = "output";
const VAR_PREFIX: &str = "VAR_";

static WGSL_REPLACEMENTS: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "let ASSIGN_VARS_GENERATED = 0.0;",
        "let ASSIGN_OUT_COLOR_GENERATED = 0.0;",
        "let FRAGMENT_DISCARD_GENERATED = 0.0;",
    ])
    .unwrap()
});

/// Generated WGSL model shader code for a material.
#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct ShaderWgsl {
    assignments: String,
    outputs: String,
    fragment_discard: String,
}

impl ShaderWgsl {
    pub fn new(program: &ShaderProgram) -> Self {
        let assignments = generate_assignments_wgsl(program);
        let outputs = generate_outputs_wgsl(program);

        // Generate empty code if there is no discard condition.
        let fragment_discard = program
            .exprs
            .discard_condition
            .map(generate_discard_wgsl)
            .unwrap_or_default();

        Self {
            assignments,
            outputs,
            fragment_discard,
        }
    }

    pub fn create_model_shader(&self) -> String {
        let replace_with = &[&self.assignments, &self.outputs, &self.fragment_discard];

        let mut source = WGSL_REPLACEMENTS.replace_all(crate::shader::model::SOURCE, replace_with);

        // This section is only used for wgsl_to_wgpu reachability analysis and can be removed.
        if let (Some(start), Some(end)) = (
            source.find("let REMOVE_BEGIN = 0.0;"),
            source.find("let REMOVE_END = 0.0;"),
        ) {
            source.replace_range(start..end, "");
        }

        source
    }
}

fn generate_assignments_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    for (i, expr) in program.exprs.exprs.iter().enumerate() {
        write!(&mut wgsl, "let {VAR_PREFIX}{i} = ",).unwrap();
        if write_expr(&mut wgsl, expr).is_none() {
            write!(wgsl, "0.0").unwrap();
        }
        writeln!(&mut wgsl, ";",).unwrap();
    }

    wgsl
}

fn write_expr(wgsl: &mut String, expr: &OutputExpr<Operation>) -> Option<()> {
    match expr {
        OutputExpr::Value(value) => write_value(wgsl, value),
        OutputExpr::Func { op, args } => write_func(wgsl, op, args),
    }
}

fn write_value(wgsl: &mut String, value: &Value) -> Option<()> {
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
        Value::Parameter(p) => write_parameter(wgsl, p),
        Value::Texture(t) => write_texture(wgsl, t),
        Value::Attribute(a) => write_attribute(wgsl, a),
    }
}

fn write_texture(wgsl: &mut String, t: &smush_shader::Texture) -> Option<()> {
    write_texture_inner(wgsl, &t.name, &t.texcoords)?;
    write_channel(wgsl, t.channel);
    Some(())
}

fn write_texture_inner(wgsl: &mut String, name: &str, texcoords: &[usize]) -> Option<()> {
    // TODO: Support remaining textures.
    match name {
        "Texture15" => {
            // TODO: shadow map sampler?
            write_sampler_2d_or_cube(wgsl, "texture_shadow", "default_sampler", texcoords)
        }
        "Texture16" => {
            // TODO: ink normal map for stages?
            error!("Unsupported texture {name}");
            None
        }
        _ => write_sampler_2d_or_cube(
            wgsl,
            &name.to_snake(),
            &name.to_snake().replace("texture", "sampler"),
            texcoords,
        ),
    }
}

fn write_sampler_2d_or_cube(
    wgsl: &mut String,
    name: &str,
    sampler: &str,
    texcoords: &[usize],
) -> Option<()> {
    match texcoords {
        [u, v] => write_sampler_2d(wgsl, name, sampler, *u, *v),
        [u, v, w] => {
            // Assume 3D textures aren't used, so UVW coordinates should always be a cube map.
            write!(
                wgsl,
                "textureSample({name}, {sampler}, vec3({VAR_PREFIX}{u}, {VAR_PREFIX}{v}, {VAR_PREFIX}{w}))",
            )
            .unwrap();
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
) -> Option<()> {
    write!(
        wgsl,
        "textureSample({name}, {sampler}, vec2({VAR_PREFIX}{u}, {VAR_PREFIX}{v}))",
    )
    .unwrap();
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
        "IN_Position" => Some("in.position"),
        "IN_Normal" => Some("in.normal"),
        "IN_Tangent" => Some("in.tangent"),
        "IN_map1" => Some("in.map1.xy"),
        "IN_uvSet" => Some("in.uv_set_uv_set1.xy"),
        "IN_uvSet1" => Some("in.uv_set_uv_set1.zw"),
        "IN_uvSet2" => Some("in.uv_set2_bake1.xy"),
        "IN_bake1" => Some("in.uv_set2_bake1.zw"),
        "IN_colorSet1" => Some("in.color_set1"),
        "IN_colorSet2" => Some("in.color_set2_combined"),
        "IN_colorSet3" => Some("in.color_set3"),
        "IN_colorSet4" => Some("in.color_set4"),
        "IN_colorSet5" => Some("in.color_set5"),
        "IN_colorSet6" => Some("in.color_set6"),
        "IN_colorSet7" => Some("in.color_set7"),
        "gl_InstanceID" => Some("0"), // TODO: instanced rendering?
        _ => {
            error!("Unrecognized attribute {a}");
            None
        }
    }?;

    write!(wgsl, "{name}").unwrap();
    write_channel(wgsl, a.channel);

    Some(())
}

fn write_parameter(wgsl: &mut String, p: &Parameter) -> Option<()> {
    if p.field == "data" {
        // Dynamic field lookups should be handled during database creation using queries.
        // Shader annotation can't handle cases like indexing by gl_InstanceID.
        error!("Unsupported dynamic uniform field {p}");
        return None;
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
                    error!("Unrecognized field {}", p.field);
                    return None;
                }
            } else {
                error!("Unrecognized field {}", p.field);
                return None;
            }
        }
        "PerObject" => {
            write!(wgsl, "per_object.{}", p.field.to_snake()).unwrap();
            write_index(wgsl, p.index);
            write_channel(wgsl, p.channel);
        }
        "ForPass" => {
            write!(wgsl, "for_pass.{}", p.field.to_snake()).unwrap();
            write_index(wgsl, p.index);
            write_channel(wgsl, p.channel);
        }
        "PerFrame" => {
            let field = match p.field.as_str() {
                "g_IBL_ColorGain" => "g_ibl_color_gain".to_string(),
                "g_IBL_Scale" => "g_ibl_scale".to_string(),
                f => f.to_snake(),
            };
            write!(wgsl, "per_frame.{field}").unwrap();
            write_index(wgsl, p.index);
            write_channel(wgsl, p.channel);
        }
        "nuPerViewCBuffer" => {
            let field = match p.field.as_str() {
                "inverseScreenSize2D" => "inverse_screen_size_2d".to_string(),
                "rtScaleFactor3d" => "rt_scale_factor_3d".to_string(),
                f => f.to_snake(),
            };
            write!(wgsl, "per_view.{field}").unwrap();
            write_index(wgsl, p.index);
            write_channel(wgsl, p.channel);
        }
        "nuPerWorldCBuffer" => {
            write!(wgsl, "per_world.{}", p.field.to_snake()).unwrap();
            write_index(wgsl, p.index);
            write_channel(wgsl, p.channel);
        }
        _ => {
            error!("Unrecognized uniform {p}");
            return None;
        }
    }
    Some(())
}

fn write_func(wgsl: &mut String, op: &Operation, args: &[usize]) -> Option<()> {
    let arg0 = args.first();
    let arg1 = args.get(1);
    let arg2 = args.get(2);

    let a = VAR_PREFIX;
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
    }
    Some(())
}

fn write_index(wgsl: &mut String, i: Option<usize>) {
    if let Some(i) = i {
        write!(wgsl, "[{VAR_PREFIX}{i}]").unwrap();
    }
}

fn write_channel(wgsl: &mut String, c: Option<char>) {
    if let Some(c) = c {
        write!(wgsl, ".{c}").unwrap();
    }
}

fn generate_outputs_wgsl(program: &ShaderProgram) -> String {
    let mut wgsl = String::new();

    for c in "xyzw".chars() {
        if let Some(i) = program
            .exprs
            .output_dependencies
            .get(&format_smolstr!("OUT_Color.{c}"))
        {
            writeln!(&mut wgsl, "{OUT_VAR}.{c} = {VAR_PREFIX}{i};").unwrap()
        }
    }

    wgsl
}

fn generate_discard_wgsl(condition_expr_index: usize) -> String {
    formatdoc! {"
        if {VAR_PREFIX}{condition_expr_index} {{
            discard;
        }}
    "}
}
