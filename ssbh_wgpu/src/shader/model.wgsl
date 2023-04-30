struct CameraTransforms {
    model_view_matrix: mat4x4<f32>,
    mvp_matrix: mat4x4<f32>,
    mvp_inv_matrix: mat4x4<f32>,
    camera_pos: vec4<f32>,
    screen_dimensions: vec4<f32>, // width, height, scale, _
};

// TODO: How to handle alignment?
struct RenderSettings {
    debug_mode: vec4<u32>,
    render_uv_pattern: vec4<u32>,
    transition_material: vec4<u32>,
    transition_factor: vec4<f32>,
    render_diffuse: vec4<u32>,
    render_specular: vec4<u32>,
    render_emission: vec4<u32>,
    render_rim_lighting: vec4<u32>,
    render_shadows: vec4<u32>,
    render_bloom: vec4<u32>,
    render_vertex_color: vec4<u32>,
    scale_vertex_color: vec4<u32>,
    render_rgba: vec4<f32>,
    render_nor: vec4<u32>,
    render_prm: vec4<u32>,
};

// Stage lighting is stored in nuanmb files like light00.nuanmb
// TODO: Does Smash Ultimate support shadow casting from multiple lights?
struct Light {
    // Store combined CustomVector0 and CustomFloat0
    color: vec4<f32>,
    // Convert quaternions to direction vectors.
    direction: vec4<f32>,
    transform: mat4x4<f32>
}

struct SceneAttributesForShaderFx {
    custom_boolean: array<vec4<u32>, 20>,
    custom_vector: array<vec4<f32>, 64>,
    custom_float: array<vec4<f32>, 20>,
};

// TODO: What is the upper limit on light sets?
// TODO: How to decide on which light is used for shadow casting?
struct StageUniforms {
    light_chr: Light,
    light_stage: array<Light, 8>,
    scene_attributes: SceneAttributesForShaderFx
};

// Bind groups are ordered by how frequently they change for performance.
// TODO: Is it worth actually optimizing this on the CPU side?
@group(0) @binding(0)
var<uniform> camera: CameraTransforms;

@group(0) @binding(1)
var texture_shadow: texture_2d<f32>;
@group(0) @binding(2)
var default_sampler: sampler;

@group(0) @binding(4)
var<uniform> render_settings: RenderSettings;

@group(0) @binding(5)
var<uniform> stage_uniforms: StageUniforms;

@group(0) @binding(6)
var uv_pattern: texture_2d<f32>;

struct PerModel {
    light_set_index: vec4<u32> // is_stage, light_set, 0, 0
}

@group(1) @binding(0)
var<uniform> per_model: PerModel;

// TODO: Is there a better way of organizing this?
// TODO: How many textures can we have?
@group(2) @binding(0)
var texture0: texture_2d<f32>;
@group(2) @binding(1)
var sampler0: sampler;

@group(2) @binding(2)
var texture1: texture_2d<f32>;
@group(2) @binding(3)
var sampler1: sampler;

@group(2) @binding(4)
var texture2: texture_cube<f32>;
@group(2) @binding(5)
var sampler2: sampler;

@group(2) @binding(6)
var texture3: texture_2d<f32>;
@group(2) @binding(7)
var sampler3: sampler;

@group(2) @binding(8)
var texture4: texture_2d<f32>;
@group(2) @binding(9)
var sampler4: sampler;

@group(2) @binding(10)
var texture5: texture_2d<f32>;
@group(2) @binding(11)
var sampler5: sampler;

@group(2) @binding(12)
var texture6: texture_2d<f32>;
@group(2) @binding(13)
var sampler6: sampler;

@group(2) @binding(14)
var texture7: texture_cube<f32>;
@group(2) @binding(15)
var sampler7: sampler;

@group(2) @binding(16)
var texture8: texture_cube<f32>;
@group(2) @binding(17)
var sampler8: sampler;

@group(2) @binding(18)
var texture9: texture_2d<f32>;
@group(2) @binding(19)
var sampler9: sampler;

@group(2) @binding(20)
var texture10: texture_2d<f32>;
@group(2) @binding(21)
var sampler10: sampler;

@group(2) @binding(22)
var texture11: texture_2d<f32>;
@group(2) @binding(23)
var sampler11: sampler;

@group(2) @binding(24)
var texture12: texture_2d<f32>;
@group(2) @binding(25)
var sampler12: sampler;

@group(2) @binding(26)
var texture13: texture_2d<f32>;
@group(2) @binding(27)
var sampler13: sampler;

@group(2) @binding(28)
var texture14: texture_2d<f32>;
@group(2) @binding(29)
var sampler14: sampler;

// TODO: use naming convention to indicate frequency like PerMaterial
// Align everything to 16 bytes to avoid alignment issues.
// Smash Ultimate's shaders also use this alignment.
// TODO: Investigate std140/std430
// TODO: Does wgsl/wgpu require a specific layout/alignment?
struct PerMaterial {
    custom_vector: array<vec4<f32>, 64>,
    // TODO: Place the has_ values in an unused vector component?
    custom_boolean: array<vec4<u32>, 20>,
    custom_float: array<vec4<f32>, 20>,
    has_boolean: array<vec4<u32>, 20>,
    has_float: array<vec4<u32>, 20>,
    has_texture: array<vec4<u32>, 19>,
    has_vector: array<vec4<u32>, 64>,
    has_color_set1234: vec4<u32>,
    has_color_set567: vec4<u32>,
    shader_settings: vec4<u32>, // discard, premultiplied, 0, 0
    lighting_settings: vec4<u32>, // lighting, sh, receives_shadow, 0
    shader_complexity: vec4<f32>
};

@group(2) @binding(30)
var<uniform> per_material: PerMaterial;

struct VertexInput0 {
    @location(0) position0: vec4<f32>,
    @location(1) normal0: vec4<f32>,
    @location(2) tangent0: vec4<f32>,
};

// We can safely assume 16 available locations.
// Pack attributes to avoid going over the attribute limit.
struct VertexInput1 {
    @location(3) map1_uvset: vec4<f32>,
    @location(4) uv_set1_uv_set2: vec4<f32>,
    @location(5) bake1: vec4<f32>,
    @location(6) color_set1: vec4<f32>,
    @location(7) color_set2_combined: vec4<f32>,
    @location(8) color_set3: vec4<f32>,
    @location(9) color_set4: vec4<f32>,
    @location(10) color_set5: vec4<f32>,
    @location(11) color_set6: vec4<f32>,
    @location(12) color_set7: vec4<f32>,
};

// TODO: This will need to be reworked at some point.
// TODO: The in game shaders combine fog and vertex color into IN_FinalGain and IN_FinalOffset.
// TODO: Some shaders use IN_VertexLightMap like dracula castle clock tower.
// TODO: Create a separate vertex shader for debug shading?
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) map1: vec4<f32>,
    @location(4) uv_set_uv_set1: vec4<f32>,
    @location(5) uv_set2_bake1: vec4<f32>,
    @location(6) color_set1: vec4<f32>,
    @location(7) color_set2_combined: vec4<f32>,
    @location(8) color_set3: vec4<f32>,
    @location(9) color_set4: vec4<f32>,
    @location(10) color_set5: vec4<f32>,
    @location(11) color_set6: vec4<f32>,
    @location(12) color_set7: vec4<f32>,
    @location(13) light_position: vec4<f32>,
    @location(14) sh_lighting: vec4<f32>,
};

struct VertexOutputInvalid {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec4<f32>,
};

fn Blend(a: vec3<f32>, b: vec4<f32>) -> vec3<f32> {
    // CustomBoolean11 toggles additive vs alpha blending.
    if (per_material.custom_boolean[11].x == 1u) {
        return a.rgb + b.rgb * b.a;
    } else {
        return mix(a.rgb, b.rgb, b.a);
    }
}

fn TransformUv(uv: vec2<f32>, transform: vec4<f32>) -> vec2<f32>
{
    // TODO: UV automatic scrolling animations?

    // TODO: dUdV Map.
    // Remap [0,1] to [-1,1].
    // let textureOffset = textureSample(texture4, sampler4, uv * 2.0).xy * 2.0 - 1.0;
    // result = result + textureOffset * per_material.custom_float[4].x;

    // UV transform code ported from Mario's eye shader in game.
    // TODO: Is it worth porting the checks for NaN and 0.0 on transform.xy?
    let x = transform.x * (uv.x - transform.z);
    let y = 1.0 - transform.y * (1.0 - uv.y - transform.w);
    return vec2(x, y);
}

// TODO: Rework texture blending to match the in game behavior.
// The game usually uses white for missing required textures.
// We use a single shader for all possible shaders.
// This requires a conditional check for each texture to render correctly.
// TODO: Ignore textures not used by the shader?
// This could probably be loaded from Rust as has_attribute & requires_attribute.
fn GetEmissionColor(uv1: vec2<f32>, uv2: vec2<f32>) -> vec4<f32> {
    var emissionColor = vec4(0.0, 0.0, 0.0, 1.0);

    if (per_material.has_texture[5].x == 1u) {
        emissionColor = textureSample(texture5, sampler5, uv1);
    }

    if (per_material.has_texture[14].x == 1u) {
        let emission2Color = textureSample(texture14, sampler14, uv2);
        return vec4(Blend(emissionColor.rgb, emission2Color), emissionColor.a);
    }

    return emissionColor;
}

fn GetAlbedoColor(uv1: vec2<f32>, uv2: vec2<f32>, uv3: vec2<f32>, R: vec3<f32>, colorSet5: vec4<f32>) -> vec4<f32>
{
    let uvLayer1 = uv1;
    let uvLayer2 = uv2;
    let uvLayer3 = uv3;

    var outRgb = vec3(0.0);
    var outAlpha = 1.0;

    // TODO: Research the other color channels of colorSet5.
    // colorSet5 asks as an additional alpha mask for texture blending.
    // TODO: Battlefield waterfalls and delfino volcano use channels differently?
    var difLayer1Mask = 1.0;
    var colLayer2Mask = 1.0;
    if (per_material.has_color_set567.x == 1u && render_settings.render_vertex_color.x == 1u) {
        difLayer1Mask = colorSet5.x;
        colLayer2Mask = colorSet5.w;
    }

    // TODO: Do additional layers affect alpha?
    if (per_material.has_texture[0].x == 1u) {
        let albedoColor = textureSample(texture0, sampler0, uvLayer1);
        outRgb = albedoColor.rgb;
        outAlpha = albedoColor.a;
    }

    // TODO: Refactor blend to take RGB and w separately?
    if (per_material.has_texture[1].x == 1u) {
        let albedoColor2 = textureSample(texture1, sampler1, uvLayer2);
        outRgb = Blend(outRgb, albedoColor2 * vec4(1.0, 1.0, 1.0, colLayer2Mask));
    }

    // Materials won't have col and diffuse cube maps.
    if (per_material.has_texture[8].x == 1u) {
        // TODO: Just return early here?
        outRgb = textureSample(texture8, sampler8, R).rgb;
    }

    if (per_material.has_texture[10].x == 1u) {
        let diffuseColor1 = textureSample(texture10, sampler10, uvLayer1);
        outRgb = Blend(outRgb, diffuseColor1 * vec4(1.0, 1.0, 1.0, difLayer1Mask));
    }
    if (per_material.has_texture[11].x == 1u) {
        let diffuseColor2 = textureSample(texture11, sampler11, uvLayer2);
        outRgb = Blend(outRgb, diffuseColor2);
    }
    if (per_material.has_texture[12].x == 1u) {
        // TODO: Is the blending always additive?
        outRgb = outRgb + textureSample(texture12, sampler12, uvLayer3).rgb;
    }

    return vec4(outRgb, outAlpha);
}

fn GetAlbedoColorFinal(albedoColor: vec4<f32>) -> vec3<f32>
{
    var albedoColorFinal = albedoColor.rgb;

    // Color multiplier param.
    // TODO: Check all channels?
    if (per_material.has_vector[13].x == 1u) {
        albedoColorFinal = albedoColorFinal * per_material.custom_vector[13].rgb;
    }

    // TODO: Wiifit stage model color.
    // if (hasCustomVector44 == 1u) {
    //     albedoColorFinal = per_material.custom_vector[44].rgb + per_material.custom_vector[45].rgb;
    // }

    return albedoColorFinal;
}

fn GetBitangent(normal: vec3<f32>, tangent: vec3<f32>, bitangent_sign: f32) -> vec3<f32>
{
    // Ultimate flips the bitangent before using it for normal mapping.
    return cross(normal.xyz, tangent.xyz) * bitangent_sign * -1.0;
}

fn GetBumpMapNormal(normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>, norColor: vec4<f32>) -> vec3<f32>
{
    // Remap the normal map to the correct range.
    let x = 2.0 * norColor.x - 1.0;
    let y = 2.0 * norColor.y - 1.0;

    // Calculate z based on the fact that x*x + y*y + z*z = 1.
    // Clamp to ensure z is positive.
    let z = sqrt(max(1.0 - (x * x) + (y * y), 0.001));

    // Normal mapping is a change of basis using the TBN vectors.
    let nor = vec3(x, y, z);
    let newNormal = tangent * nor.x + bitangent * nor.y + normal * nor.z;
    return normalize(newNormal);
}

fn GetLight() -> Light {
    // TODO: How expensive is this?
    // TODO: Is this worth moving to the CPU?
    if (per_model.light_set_index.x == 0u) {
        return stage_uniforms.light_chr;
    } else {
        switch (per_model.light_set_index.y) {
            case 0u: {
                return stage_uniforms.light_stage[0];
            }
            case 1u: {
                return stage_uniforms.light_stage[1];
            }
            case 2u: {
                return stage_uniforms.light_stage[2];
            }
            case 3u: {
                return stage_uniforms.light_stage[3];
            }
            case 4u: {
                return stage_uniforms.light_stage[4];
            }
            case 5u: {
                return stage_uniforms.light_stage[5];
            }
            case 6u: {
                return stage_uniforms.light_stage[6];
            }
            case 7u: {
                return stage_uniforms.light_stage[7];
            }
            default: {
                return stage_uniforms.light_stage[7];
            }
        }
    }
}

fn DiffuseTerm(
    bake1: vec2<f32>,
    albedo: vec3<f32>,
    nDotL: f32,
    shLighting: vec3<f32>,
    ao: f32,
    sss_color: vec3<f32>,
    sss_smooth_factor: f32,
    sss_blend: f32,
    shadow: f32,
    colorSet2: vec4<f32>) -> vec3<f32>
{
    // TODO: This can be cleaned up.
    var directShading = albedo * max(nDotL, 0.0);

    // TODO: nDotL is a vertex attribute for skin shading.

    // Diffuse shading is remapped to be softer.
    // Multiplying be a constant and clamping affects the "smoothness".
    var nDotLSkin = nDotL * sss_smooth_factor;
    nDotLSkin = clamp(nDotLSkin * 0.5 + 0.5, 0.0, 1.0);
    // TODO: Why is there an extra * sssBlend term here?
    let skinShading = sss_color * sss_blend * nDotLSkin;

    // TODO: How many PI terms are there?
    // TODO: Skin shading looks correct without the PI term?
    directShading = mix(directShading / 3.14159, skinShading, sss_blend);

    var directLight = vec3(0.0);
    if (per_material.lighting_settings.x == 1u) {
        directLight = GetLight().color.rgb * directShading;
    }
    var ambientTerm = (shLighting * ao);

    // TODO: Does Texture3 also affect specular?
    if (per_material.has_texture[3].x == 1u) {
        ambientTerm = ambientTerm * textureSample(texture3, sampler3, bake1).rgb;
    }

    if (per_material.has_texture[9].x == 1u) {
        // The alpha channel masks direct lighting to act as baked shadows.
        let bakedLitColor = textureSample(texture9, sampler9, bake1).rgba;
        directLight = directLight * bakedLitColor.a;

        // The RGB channels control the ambient lighting color.
        // Baked lighting maps are not affected by ambient occlusion.
        ambientTerm = ambientTerm + (bakedLitColor.rgb * 8.0);
    }

    // Assume the mix factor is 0.0 if the material doesn't have CustomVector11.
    ambientTerm = ambientTerm * mix(albedo, sss_color, sss_blend);

    var result = directLight * shadow + ambientTerm;

    // Baked stage lighting.
    // TODO: How is this different from colorSet1?
    // TODO: Check the king model on zelda_tower.
    if (per_material.has_color_set1234.y == 1u && render_settings.render_vertex_color.x == 1u) {
       result = result * colorSet2.rgb;
    }

    return result;
}

// Schlick fresnel approximation.
fn FresnelSchlick(cosTheta: f32, f0: vec3<f32>) -> vec3<f32>
{
    return f0 + (1.0 - f0) * pow(1.0 - cosTheta, 5.0);
}

// Ultimate uses something similar to the schlick geometry masking term.
// http://cwyman.org/code/dxrTutors/tutors/Tutor14/tutorial14.md.html
fn SchlickMaskingTerm(nDotL: f32, nDotV: f32, a2: f32) -> f32
{
    let PI = 3.14159;
    let k = a2 * 0.5;
    let gV = 1.0 / (nDotV * (1.0 - k) + k);
    // TODO: This is nDotL/PI in the shader?
    let gL = 1.0 / ((nDotL/PI) * (1.0 - k) + k);
    return gV * gL;
}

// Ultimate shaders use a mostly standard GGX BRDF for specular.
// http://graphicrants.blogspot.com/2013/08/specular-brdf-reference.html
fn Ggx(nDotH: f32, nDotL: f32, nDotV: f32, roughness: f32) -> f32
{
    // Clamp to 0.01 to prevent divide by 0.
    let a = max(roughness, 0.01) * max(roughness, 0.01);
    let a2 = a*a;
    let PI = 3.14159;
    let nDotH2 = nDotH * nDotH;

    let denominator = ((nDotH2) * (a2 - 1.0) + 1.0);
    let ggx = a2 / (denominator * denominator);
    let shadowing = SchlickMaskingTerm(nDotL, nDotV, a2);
    // TODO: why do we need to divide by an extra PI here?
    return nDotL/PI * ggx * shadowing / PI / PI;
}

fn GgxAnisotropic(nDotH: f32, h: vec3<f32>, nDotL: f32, nDotV: f32, tangent: vec3<f32>, bitangent: vec3<f32>, roughness: f32, anisotropy: f32) -> f32
{
    // Clamp to 0.01 to prevent divide by 0.
    let roughnessX = max(max(roughness, 0.01) * anisotropy, 0.01);
    let roughnessY = max(max(roughness, 0.01) / anisotropy, 0.01);

    let roughnessX2 = roughnessX * roughnessX;
    let roughnessY2 = roughnessY * roughnessY;

    let roughnessX4 = roughnessX2 * roughnessX2;
    let roughnessY4 = roughnessY2 * roughnessY2;

    // TODO: Why does this look too smooth?
    // TODO: These depend on normals?
    let xDotH = dot(bitangent, h); // TODO: Is this right?
    let xTerm = (xDotH * xDotH) / roughnessX4;

    let yDotH = dot(tangent, h); // TODO: Is this right?
    let yTerm = (yDotH * yDotH) / roughnessY4;

    // TODO: Check this section of code.
    let nDotHClamp = clamp(nDotH, 0.0, 1.0);
    let denominator = xTerm + yTerm + nDotHClamp*nDotHClamp;

    let normalization = roughnessX2 * roughnessY2 * denominator*denominator;

    // TODO: Optimize GGX functions and share code.
    // TODO: constants in WGSL?
    let PI = 3.14159;
    let a = max(roughness, 0.01) * max(roughness, 0.01);
    let a2 = a*a;
    let shadowing = SchlickMaskingTerm(nDotL, nDotV, a2);
    // TODO: Why do we need to divide by an extra PI here?
    return nDotL/PI * shadowing / normalization / PI / PI;
}

fn SpecularBrdf(tangent: vec4<f32>, bitangent: vec3<f32>, nDotH: f32, nDotL: f32, nDotV: f32, halfAngle: vec3<f32>, roughness: f32) -> f32
{
    // TODO: How to calculate tangents and bitangents for prm.a anisotropic rotation?
    // The two BRDFs look very different so don't just use anisotropic for everything.
    if (per_material.has_float[10].x == 1u) {
        return GgxAnisotropic(nDotH, halfAngle, nDotL, nDotV, tangent.xyz, bitangent, roughness, per_material.custom_float[10].x);
    } else {
        return Ggx(nDotH, nDotL, nDotV, roughness);
    }
}

fn SpecularTerm(tangent: vec4<f32>, bitangent: vec3<f32>, nDotH: f32, nDotL: f32, nDotV: f32, halfAngle: vec3<f32>,
    roughness: f32, specularIbl: vec3<f32>, kDirect: vec3<f32>, kIndirect: vec3<f32>) -> vec3<f32>
{
    var directSpecular = vec3(4.0);
    directSpecular = directSpecular * SpecularBrdf(tangent, bitangent, nDotH, nDotL, nDotV, halfAngle, roughness);
    if (per_material.has_boolean[3].x == 1u && per_material.custom_boolean[3].x == 0u) {
        directSpecular = vec3(0.0);
    }

    var indirectSpecular = specularIbl;
    if (per_material.has_boolean[4].x == 1u && per_material.custom_boolean[4].x == 0u) {
        indirectSpecular = vec3(0.0);
    }

    let specularTerm = directSpecular * kDirect + indirectSpecular * kIndirect;

    return specularTerm;
}

fn EmissionTerm(emissionColor: vec4<f32>) -> vec3<f32>
{
    var result = emissionColor.rgb;
    // TODO: Check all channels?
    if (per_material.has_vector[3].x == 1u) {
        result = result * per_material.custom_vector[3].rgb;
    }

    return result;
}

fn GetF0FromIor(ior: f32) -> f32
{
    return pow((1.0 - ior) / (1.0 + ior), 2.0);
}

// TODO: Is this just a regular lighting term?
// TODO: Does this depend on the light direction and intensity?
fn GetRimBlend(baseColor: vec3<f32>, diffusePass: vec3<f32>, nDotV: f32, nDotL: f32, occlusion: f32, vertexAmbient: vec3<f32>) -> vec3<f32>
{
    var rimColor = per_material.custom_vector[14].rgb * stage_uniforms.scene_attributes.custom_vector[8].rgb;

    // TODO: How is the overall intensity controlled?
    // Hardcoded shader constant.
    let rimIntensity = 0.2125999927520752;
    // rimColor *= rimIntensity;

    // TODO: There some sort of directional lighting that controls the intensity of this effect.
    // This appears to be lighting done in the vertex shader.
    rimColor = rimColor * vertexAmbient;

    // TODO: Black edges for large blend values?
    // Edge tint.
    rimColor = rimColor * clamp(mix(vec3(1.0), diffusePass, per_material.custom_float[8].x), vec3(0.0), vec3(1.0));

    let fresnel = pow(1.0 - nDotV, 5.0);
    var rimBlend = fresnel * stage_uniforms.scene_attributes.custom_vector[8].w * per_material.custom_vector[14].w * 0.6;
    rimBlend = rimBlend * occlusion;

    // TODO: Rim lighting is directional?
    // TODO: What direction vector is this based on?
    rimBlend = rimBlend * nDotL;

    let result = mix(baseColor, rimColor, clamp(rimBlend, 0.0, 1.0));
    return result;
}

fn RoughnessToLod(roughness: f32) -> f32
{
    // Adapted from decompiled shader source.
    // Applies a curves adjustment to roughness.
    // Clamp roughness to avoid divide by 0.
    let roughnessClamped = max(roughness, 0.01);
    let a = (roughnessClamped * roughnessClamped);
    return log2((1.0 / a) * 2.0 - 2.0) * -0.4545 + 4.0;
}

fn GetAngleFade(nDotV: f32, ior: f32, specularf0: f32) -> f32
{
    // CustomFloat19 defines the IOR for a separate fresnel based fade.
    // The specular f0 value is used to set the minimum opacity.
    let f0AngleFade = GetF0FromIor(ior + 1.0);
    let facingRatio = FresnelSchlick(nDotV, vec3(f0AngleFade)).x;
    return max(facingRatio, specularf0);
}

fn GetF0FromSpecular(specular: f32) -> f32
{
    // Specular gets remapped from [0.0,1.0] to [0.0,0.2].
    // The value is 0.16*0.2 = 0.032 if the PRM alpha is ignored.
    if (per_material.has_boolean[1].x == 1u && per_material.custom_boolean[1].x == 0u) {
        return 0.16 * 0.2;
    }

    return specular * 0.2;
}

// Shadow mapping.
fn GetShadow(light_position: vec4<f32>) -> f32
{
    // compensate for the Y-flip difference between the NDC and texture coordinates
    let flipCorrection = vec2(0.5, -0.5);
    // compute texture coordinates for shadow lookup
    let projCorrection = 1.0 / light_position.w;
    var light_local = light_position.xy * flipCorrection * projCorrection + vec2(0.5, 0.5);
    // Clamp the UVs since the sampler is shared with a repeat sampler.
    light_local = clamp(light_local, vec2(0.0), vec2(1.0));

    // TODO: This assumes depth is in the range 0.0 to 1.0 in the texture.
    let currentDepth = light_position.z * projCorrection;

    // Translated variance shadow mapping from in game.
    let m1 = textureSample(texture_shadow, default_sampler, light_local).r;
    let m2 = textureSample(texture_shadow, default_sampler, light_local).g;
    let sigma2 = clamp(m2 - m1*m1 + 0.0001, 0.0, 1.0);
    let tDif = max(currentDepth - m1, 0.0);
    // Approximate Pr(x >= t) using one of Chebychev's inqequalities.
    var shadow = sigma2 / (sigma2 + tDif*tDif);
    // TODO: Why is there a pow(shadow, 4.0) in game?
    shadow = pow(shadow, 4.0);
    return shadow;
}

fn VertexLightMap(colorSet2: vec4<f32>, colorSet2_1: vec4<f32>, colorSet2_2: vec4<f32>, colorSet2_3: vec4<f32>) -> vec4<f32> {
    // TODO: How to incorporate this into the actual code without exceeding attribute count limits?
    let lightMapMixWeight = vec4(1.0, 0.0, 0.0, 0.0);
    let lightX = colorSet2 * colorSet2 * 7.0;
    let lightY = colorSet2_1 * colorSet2_1 * 7.0;
    let lightZ = colorSet2_2 * colorSet2_2 * 7.0;
    let lightW = colorSet2_3 * colorSet2_3 * 7.0;

    return lightX * lightMapMixWeight.x + lightY * lightMapMixWeight.y + lightZ * lightMapMixWeight.z + lightW * lightMapMixWeight.w;
}

@vertex
fn vs_main(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> VertexOutput {
    var out: VertexOutput;
    out.position = buffer0.position0;
    out.clip_position = camera.mvp_matrix * vec4(buffer0.position0.xyz, 1.0);
    // Assume the z offset defaults to 0.0.
    out.clip_position.z = out.clip_position.z - per_material.custom_float[16].x;
    out.normal = buffer0.normal0;
    out.tangent = buffer0.tangent0;

    // TODO: How to update the SH lighting for each mesh?
    // The in game shaders just add these coefficients as uniforms.
    // TODO: Create a PerMesh buffer with SH coefficients?
    // This could just use PerModel for now based on the model type in the xmb.
    // Stages and fighters use different SH coefficients.
    // TODO: The easiest is to just update a global SH Coefficients buffer for now.
    out.sh_lighting = vec4(0.0);
    if (per_material.lighting_settings.y == 1u) {
        let shNormal = vec4(normalize(buffer0.normal0.xyz), 1.0);
        let shAmbientR = dot(shNormal, vec4(0.14186, 0.04903, -0.082, 1.11054));
        let shAmbientG = dot(shNormal, vec4(0.14717, 0.03699, -0.08283, 1.11036));
        let shAmbientB = dot(shNormal, vec4(0.1419, 0.04334, -0.08283, 1.11018));
        out.sh_lighting = vec4(shAmbientR, shAmbientG, shAmbientB, 0.0);
    }

    // TODO: Also apply transforms to the debug shader?
    var uvTransform1 = vec4(1.0, 1.0, 0.0, 0.0);
    // TODO: Check all channels?
    if (per_material.has_vector[6].x == 1u) {
        uvTransform1 = per_material.custom_vector[6];
    }

    var uvTransform2 = vec4(1.0, 1.0, 0.0, 0.0);
    if (per_material.has_vector[31].x == 1u) {
        uvTransform2 = per_material.custom_vector[31];
    }

    var uvTransform3 = vec4(1.0, 1.0, 0.0, 0.0);
    if (per_material.has_vector[32].x == 1u) {
        uvTransform3 = per_material.custom_vector[32];
    }

    var uvTransformDualNormal = vec4(1.0, 1.0, 0.0, 0.0);
    if (per_material.has_vector[34].x == 1u) {
        uvTransformDualNormal = per_material.custom_vector[34];
    }

    var map1 = TransformUv(buffer1.map1_uvset.xy, uvTransform1);
    var map1_dual = TransformUv(buffer1.map1_uvset.xy, uvTransformDualNormal);

    // Sprite sheet params.
    // Perform this in the fragment shader to avoid affecting debug modes.
    if (per_material.has_vector[18].x == 1u) {
        let columnCount = round(per_material.custom_vector[18].x);
        let rowCount = round(per_material.custom_vector[18].y);
        let spriteCount = round(per_material.custom_vector[18].w);
        var spriteIndex = 1.0;

        if (per_material.custom_boolean[9].x == 1u) {
            map1 /= round(per_material.custom_vector[18].xy);
            spriteIndex = (round(per_material.custom_vector[18].z) - 1.0) % spriteCount;
        }
        // else {
        //     spriteIndex = (round(per_material.custom_vector[18].z) / floor(currentFrame)) % spriteCount;
        // }

        map1.x += (1.0 / columnCount) * (spriteIndex % columnCount);
        map1.y += (1.0 / rowCount) * floor(spriteIndex / columnCount);
    }

    let uvSet = TransformUv(buffer1.map1_uvset.zw, uvTransform2);
    let uvSet1 = TransformUv(buffer1.uv_set1_uv_set2.xy, uvTransform3);
    // TODO: Transform for uvSet2?
    let uvSet2 = TransformUv(buffer1.uv_set1_uv_set2.xy, uvTransform3);

    out.map1 = vec4(map1, map1_dual);
    out.uv_set_uv_set1 = vec4(uvSet, uvSet1);
    out.uv_set2_bake1 = vec4(uvSet2, buffer1.bake1.xy);

    if (render_settings.scale_vertex_color.x == 1u) {
        // Apply color scaling since the attribute is stored as four bytes.
        // This allows values greater than the normal 0.0 to 1.0 range.
        out.color_set1 = buffer1.color_set1 * 2.0;
        out.color_set2_combined = (buffer1.color_set2_combined * buffer1.color_set2_combined) * 7.0;
        out.color_set3 = buffer1.color_set3 * 2.0;
        out.color_set4 =  buffer1.color_set4 * 2.0;
        out.color_set5 = buffer1.color_set5 * 3.0;
        out.color_set6 = buffer1.color_set6 * 3.0;
        out.color_set7 = buffer1.color_set7;
    } else {
        // It can be useful to see the unmodified values for debug modes.
        out.color_set1 = buffer1.color_set1;
        out.color_set2_combined = buffer1.color_set2_combined;
        out.color_set3 = buffer1.color_set3;
        out.color_set4 =  buffer1.color_set4;
        out.color_set5 = buffer1.color_set5;
        out.color_set6 = buffer1.color_set6;
        out.color_set7 = buffer1.color_set7;
    }

    out.light_position = GetLight().transform * vec4(buffer0.position0.xyz, 1.0);
    return out;
}

@vertex
fn vs_depth(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> @builtin(position) vec4<f32> {
    return GetLight().transform * vec4(buffer0.position0.xyz, 1.0);
}

@vertex
fn vs_uv(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> @builtin(position) vec4<f32> {
    // TODO: Add an option to select the UV map.
    let uv = vec2(buffer1.map1_uvset.x, 1.0 - buffer1.map1_uvset.y);
    return vec4(uv, 0.0, 1.0);
}

fn ScreenCheckerBoard(screenPosition: vec2<f32>) -> f32
{
    // Port of in game shader code for screen checkerboard.
    let x = screenPosition.x - 16.0 * floor(screenPosition.x / 16.0);
    let y = screenPosition.y - 16.0 * floor(screenPosition.y / 16.0);

    if ((x <= 8.0 && y >= 8.0) || (x >= 8.0 && y < 8.0)) {
        return 1.0;
    } else {
        return 0.0;
    }
}

fn plasma_colormap(x: f32) -> vec3<f32> {
    // TODO: Just use a uniform array for this instead?
    // Colormaps generated from tables provided at
    // https://www.kennethmoreland.com/color-advice/
    let plasma8 = array(
        vec3(0.05038205347059877,0.029801736499741757,0.5279751010495176),
        vec3(0.32784692303604196,0.0066313933705768055,0.6402853293744383),
        vec3(0.5453608398097519,0.03836817688235455,0.6472432548304646),
        vec3(0.7246542772727967,0.1974236709187686,0.5379281037132716),
        vec3(0.8588363515132411,0.35929521887338184,0.407891799954962),
        vec3(0.9557564842476064,0.5338287173328614,0.2850080723374925),
        vec3(0.9945257260387773,0.7382691276441445,0.16745985897148677),
        vec3(0.9400151278782742,0.9751557856205376,0.131325887773911),
    );

    // Use an array to avoid adding another texture.
    let position = x * 7.0;
    let index = i32(position);
    var low = plasma8[0];
    var high = plasma8[0];

    // Workaround for WGSL only allowing constant array indices.
    switch (index) {
        case 0: {
            low = plasma8[0];
            high = plasma8[1];
            break;
        }
        case 1: {
            low = plasma8[1];
            high = plasma8[2];
            break;
        }
        case 2: {
            low = plasma8[2];
            high = plasma8[3];
            break;
        }
        case 3: {
            low = plasma8[3];
            high = plasma8[4];
            break;
        }
        case 4: {
            low = plasma8[4];
            high = plasma8[5];
            break;
        }
        case 5: {
            low = plasma8[5];
            high = plasma8[6];
            break;
        }
        case 6: {
            low = plasma8[6];
            high = plasma8[7];
            break;
        }
        default: {
            low = plasma8[7];
            high = plasma8[7];
            break;
        }
    }

    // Interpolate between the two closest elements.
    return mix(low, high, fract(position));
}

@vertex
fn vs_main_invalid(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> VertexOutputInvalid {
    var out: VertexOutputInvalid;
    out.clip_position = camera.mvp_matrix * vec4(buffer0.position0.xyz, 1.0);
    out.position = out.clip_position;

    return out;
}

@fragment
fn fs_invalid_shader(in: VertexOutputInvalid) -> @location(0) vec4<f32> {
    let position_clip = (in.position.xy / in.position.w) * 0.5 + 0.5;
    // Account for screen dimensions and scale.
    let checker = ScreenCheckerBoard(position_clip * camera.screen_dimensions.xy / camera.screen_dimensions.z);
    return vec4(checker, 0.0, 0.0, 1.0);
}

@fragment
fn fs_invalid_attributes(in: VertexOutputInvalid) -> @location(0) vec4<f32> {
    let position_clip = (in.position.xy / in.position.w) * 0.5 + 0.5;
    // Account for screen dimensions and scale.
    let checker = ScreenCheckerBoard(position_clip * camera.screen_dimensions.xy / camera.screen_dimensions.z);
    return vec4(checker, checker, 0.0, 1.0);
}

@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO: Customize this color?
    return vec4(1.0);
}

@fragment
fn fs_selected_material(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO: Customize this color?
    // Zero alpha as a workaround to disable post processing.
    return vec4(0.0, 1.0, 1.0, 0.0);
}

@fragment
fn fs_uv() -> @location(0) vec4<f32> {
    // TODO: Customize this color?
    return vec4(1.0);
}

@fragment
fn fs_debug(in: VertexOutput) -> @location(0) vec4<f32> {
    let map1 = in.map1.xy;
    let map1_dual = in.map1.zw;
    let uvSet = in.uv_set_uv_set1.xy;
    let uvSet1 = in.uv_set_uv_set1.zw;
    let uvSet2 = in.uv_set2_bake1.xy;
    let bake1 = in.uv_set2_bake1.zw;

    let colorSet1 = in.color_set1;
    let colorSet2 = in.color_set2_combined;
    let colorSet3 = in.color_set3;
    let colorSet4 = in.color_set4;
    let colorSet5 = in.color_set5;
    let colorSet6 = in.color_set6;
    let colorSet7 = in.color_set7;

    // Normal code ported from in game.
    // This is similar to mikktspace but normalization happens in the fragment shader.
    let normal = normalize(in.normal.xyz);
    let tangent = normalize(in.tangent.xyz);
    let bitangent = GetBitangent(normal, tangent, in.tangent.w);

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);

    var reflectionVector = reflect(viewVector, normal);
    reflectionVector.y = reflectionVector.y * -1.0;

    // TODO: Apply normal maps and convert to view space.
    var fragmentNormal = normal;
    if (per_material.has_texture[4].x == 1u) {
        var nor = textureSample(texture4, sampler4, map1);
        // TODO: Simpler way to toggle channels?
        if (render_settings.render_nor.r == 0u) {
            nor.r = 0.5;
        }
        if (render_settings.render_nor.g == 0u) {
            nor.g = 0.5;
        }
        if (render_settings.render_nor.b == 0u) {
            nor.b = 0.0;
        }
        if (render_settings.render_nor.a == 0u) {
            nor.a = 1.0;
        }
        fragmentNormal = GetBumpMapNormal(normal, tangent, bitangent, nor);
    }

    var prm = vec4(0.0, 0.0, 1.0, 0.0);
    if (per_material.has_texture[6].x == 1u) {
        prm = textureSample(texture6, sampler6, map1);
    }

    // Move fake subsurface color into GetAlbedoColorFinal?
    let sssBlend = prm.r * per_material.custom_vector[30].x;
    let albedoColor = GetAlbedoColor(map1, uvSet, uvSet1, reflectionVector, colorSet5);
    var albedoColorFinal = GetAlbedoColorFinal(albedoColor);
    albedoColorFinal = mix(albedoColorFinal, per_material.custom_vector[11].rgb, sssBlend);
    let emissionColor = GetEmissionColor(map1, uvSet);

    // TODO: Some of these render modes should be gamma corrected.
    // TODO: Use more accurate gamma correction.
    var outColor = vec4(1.0);
    switch (render_settings.debug_mode.x) {
        case 1u: {
            let color = normalize(in.position.xyz) * 0.5 + 0.5;
            outColor = vec4(pow(color, vec3(2.2)), 1.0);
        }
        case 2u: {
            let color = in.normal.xyz * 0.5 + 0.5;
            outColor = vec4(pow(color, vec3(2.2)), 1.0);
        }
        case 3u: {
            let color = in.tangent.xyz * 0.5 + 0.5;
            outColor = vec4(pow(color, vec3(2.2)), in.tangent.w);
        }
        case 4u: {
            outColor = colorSet1;
        }
        case 5u: {
            outColor = colorSet2;
        }
        case 6u: {
            outColor = colorSet3;
        }
        case 7u: {
            outColor = colorSet4;
        }
        case 8u: {
            outColor = colorSet5;
        }
        case 9u: {
            outColor = colorSet6;
        }
        case 10u: {
            outColor = colorSet7;
        }
        case 11u: {
            outColor = textureSample(texture0, sampler0, map1);
        }
        case 12u: {
            outColor = textureSample(texture1, sampler1, uvSet);
        }
        case 13u: {
            outColor = textureSample(texture2, sampler2, reflectionVector);
        }
        case 14u: {
            outColor = textureSample(texture3, sampler3, bake1);
        }
        case 15u: {
            outColor = textureSample(texture4, sampler4, map1);
        }
        case 16u: {
            outColor = textureSample(texture5, sampler5, map1);
        }
        case 17u: {
            outColor = textureSample(texture6, sampler6, map1);
        }
        case 18u: {
            outColor = textureSample(texture7, sampler7, reflectionVector);
        }
        case 19u: {
            outColor = textureSample(texture8, sampler8, reflectionVector);
        }
        case 20u: {
            outColor = textureSample(texture9, sampler9, bake1);
        }
        case 21u: {
            outColor = textureSample(texture10, sampler10, map1);
        }
        case 22u: {
            outColor = textureSample(texture11, sampler11, uvSet);
        }
        case 23u: {
            outColor = textureSample(texture12, sampler12, map1);
        }
        case 24u: {
            outColor = textureSample(texture13, sampler13, map1);
        }
        case 25u: {
            outColor = textureSample(texture14, sampler14, uvSet);
        }
        // case 26u: {
        // TODO: Find a way to include this many textures?
        //     outColor = textureSample(texture16, sampler16, map1);
        // }
        case 27u: {
            if (render_settings.render_uv_pattern.x == 1u) {
                outColor = textureSample(uv_pattern, default_sampler, map1);
            } else {
                // Use fract to remap values to 0.0 to 1.0 similar to a repeat wrap mode.
                outColor = vec4(pow(fract(map1), vec2(2.2)), 1.0, 1.0);
            }
        }
        case 28u: {
            if (render_settings.render_uv_pattern.x == 1u) {
                outColor = textureSample(uv_pattern, default_sampler, bake1);
            } else {
                outColor = vec4(pow(fract(bake1), vec2(2.2)), 1.0, 1.0);
            }
        }
        case 29u: {
            if (render_settings.render_uv_pattern.x == 1u) {
                outColor = textureSample(uv_pattern, default_sampler, uvSet);
            } else {
                outColor = vec4(pow(fract(uvSet), vec2(2.2)), 1.0, 1.0);
            }
        }
        case 30u: {
            if (render_settings.render_uv_pattern.x == 1u) {
                outColor = textureSample(uv_pattern, default_sampler, uvSet1);
            } else {
                outColor = vec4(pow(fract(uvSet1), vec2(2.2)), 1.0, 1.0);
            }
        }
        case 31u: {
            if (render_settings.render_uv_pattern.x == 1u) {
                outColor = textureSample(uv_pattern, default_sampler, uvSet2);
            } else {
                outColor = vec4(pow(fract(uvSet2), vec2(2.2)), 1.0, 1.0);
            }
        }
        case 32u: {
            // Basic Shading
            let basic = 0.218 * max(dot(fragmentNormal, viewVector), 0.0);
            outColor = vec4(vec3(basic), 1.0);
        }
        case 33u: {
            // Normals
            outColor = vec4(pow(fragmentNormal.xyz * 0.5 + 0.5, vec3(2.2)), 1.0);
        }
        case 34u: {
            // Bitangents
            outColor = vec4(pow(bitangent.xyz * 0.5 + 0.5, vec3(2.2)), 1.0);
        }
        case 35u: {
            // Unlit
            outColor = vec4(albedoColorFinal.rgb + emissionColor.rgb, albedoColor.a * emissionColor.a);
        }
        case 36u: {
            // The min complexity isn't 0.0 since shaders are all non empty.
            // Normalize to the 0.0 to 1.0 range to use the full colormap.
            let min = 0.10816174646489705;
            let complexity = (per_material.shader_complexity.x - min) / (1.0 - min);
            let color = plasma_colormap(complexity);
            outColor = vec4(pow(color, vec3(2.2)), 1.0);
        }
        default: {
            outColor = vec4(1.0);
        }
    }

    // Use grayscale for single channels.
    let rgba = render_settings.render_rgba;
    if (rgba.r == 1.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4(outColor.rrr, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 1.0 && rgba.b == 0.0) {
        return vec4(outColor.ggg, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 1.0) {
        return vec4(outColor.bbb, 1.0);
    }

    if (rgba.a == 1.0 && rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4(outColor.aaa, 1.0);
    }

    return vec4(outColor.rgb * rgba.rgb, 1.0);
}

// TODO: use Rust naming conventions?
struct PbrParams {
    albedo: vec4<f32>,
    emission: vec4<f32>,
    nor: vec4<f32>,
    sss_color: vec3<f32>,
    sss_blend: f32,
    sss_smooth_factor: f32,
    // PRM
    metalness: f32,
    roughness: f32,
    ambient_occlusion: f32,
    specular_f0: f32
}

fn GetPbrParams(in: VertexOutput, viewVector: vec3<f32>, reflectionVector: vec3<f32>) -> PbrParams {
    // TODO: Create a struct for the non packed attributes.
    let map1 = in.map1.xy;
    let map1_dual = in.map1.zw;
    let uvSet = in.uv_set_uv_set1.xy;
    let uvSet1 = in.uv_set_uv_set1.zw;
    let uvSet2 = in.uv_set2_bake1.xy;
    let bake1 = in.uv_set2_bake1.zw;

    let colorSet1 = in.color_set1;
    let colorSet2 = in.color_set2_combined;
    let colorSet3 = in.color_set3;
    let colorSet4 = in.color_set4;
    let colorSet5 = in.color_set5;
    let colorSet6 = in.color_set6;
    let colorSet7 = in.color_set7;

    // TODO: Mario's eyes don't render properly for the metal/gold materials.
    var out: PbrParams;

    out.nor = vec4(0.5, 0.5, 1.0, 1.0);
    if (per_material.has_texture[4].x == 1u) {
        out.nor = textureSample(texture4, sampler4, map1);
        if (per_material.has_vector[34].x == 1u) {
            // The second layer is added to the first layer.
            // TODO: These shaders use the z channel as a normal map.
            let nor1 = out.nor.xyz;
            let nor2 = textureSample(texture4, sampler4, map1_dual).xyz;
            out.nor = vec4(nor1 + nor2 - 1.0, out.nor.w);
        }

        // TODO: Simpler way to toggle channels?
        if (render_settings.render_nor.r == 0u) {
            out.nor.r = 0.5;
        }
        if (render_settings.render_nor.g == 0u) {
            out.nor.g = 0.5;
        }
        if (render_settings.render_nor.b == 0u) {
            out.nor.b = 0.0;
        }
        if (render_settings.render_nor.a == 0u) {
            out.nor.a = 1.0;
        }
    }

    // Check if the factor is non zero to prevent artifacts.
    var transitionFactor = 0.0;
    if ((render_settings.transition_factor.x > 0.0) && (out.nor.b >= (1.0 - render_settings.transition_factor.x))) {
        transitionFactor = 1.0;
    }

    // TODO: Finish researching these values from in game.
    // TODO: Apply the metal/metamon materials.
    var transitionAlbedo = vec3(0.0);
    var transitionPrm = vec4(0.0);
    var transitionCustomVector11 = vec4(0.0);
    var transitionCustomVector30 = vec4(0.0);

    switch (render_settings.transition_material.x) {
        case 0u: {
            // Inkling's Ink.
            // TODO: Include other colors from /fighter/common/param/effect.prc?
            transitionAlbedo = vec3(0.758027, 0.115859, 0.04);
            // TODO: Ink PRM?
            transitionPrm = vec4(0.0, 0.2, 1.0, 0.16);
            transitionCustomVector11 = vec4(0.0);
            transitionCustomVector30 = vec4(0.0);
        }
        case 1u: {
            // Metal Box.
            transitionAlbedo = vec3(0.257, 0.257, 0.257);
            transitionPrm = vec4(1.0, 0.3, 1.0, 0.0);
            transitionCustomVector11 = vec4(0.0);
            transitionCustomVector30 = vec4(0.0);
        }
        case 2u: {
            // Gold (Xerneas Pokemon).
            // (0.257, 0.257, 0.257) + (0.125, 0.047, -0.234) in the shader.
            transitionAlbedo = vec3(0.382, 0.304, 0.023);
            transitionPrm = vec4(1.0, 0.3, 1.0, 0.0);
            transitionCustomVector11 = vec4(0.0);
            transitionCustomVector30 = vec4(0.0);
        }
        case 3u: {
            // Ditto Pokemon.
            transitionAlbedo = vec3(0.1694, 0.0924, 0.2002);
            transitionPrm = vec4(1.0, 0.75, 1.0, 0.032); // TODO: Roughness?
            transitionCustomVector11 = vec4(0.0); // TODO: What is this?
            transitionCustomVector30 = vec4(0.5, 4.0, 0.0, 0.0);
        }
        default: {

        }
    }

    // TODO: Combine mix with each case above?
    out.sss_color = mix(per_material.custom_vector[11].rgb, transitionCustomVector11.rgb, render_settings.transition_factor.x);
    out.sss_blend = mix(per_material.custom_vector[30].x, transitionCustomVector30.x, render_settings.transition_factor.x);
    out.sss_smooth_factor = mix(per_material.custom_vector[30].y, transitionCustomVector30.y, render_settings.transition_factor.x);

    var prm = vec4(0.0, 0.0, 1.0, 0.0);
    let hasPrm = per_material.has_texture[6].x == 1u;
    if (hasPrm) {
        prm = textureSample(texture6, sampler6, map1);
        // TODO: Simpler way to toggle channels?
        if (render_settings.render_prm.r == 0u) {
            prm.r = 0.0;
        }
        if (render_settings.render_prm.g == 0u) {
            prm.g = 1.0;
        }
        if (render_settings.render_prm.b == 0u) {
            prm.b = 1.0;
        }
        if (render_settings.render_prm.a == 0u) {
            prm.a = 0.16;
        }
    }
    // Not all channels are used for all shaders.
    // TODO: Is there a cleaner way of writing this?
    let vector47 = per_material.custom_vector[47];
    var hasVector47 = false;
    if (per_material.has_vector[47].x == 1u) {
        prm.x = vector47.x;
        hasVector47 = true;
    }
    if (per_material.has_vector[47].y == 1u) {
        prm.y = vector47.y;
        hasVector47 = true;
    }
    if (per_material.has_vector[47].z == 1u) {
        prm.z = vector47.z;
        hasVector47 = true;
    }
    if (per_material.has_vector[47].w == 1u) {
        prm.w = vector47.w;
        hasVector47 = true;
    }

    out.metalness = mix(prm.r, transitionPrm.r, transitionFactor);
    out.roughness = mix(prm.g, transitionPrm.g, transitionFactor);
    out.ambient_occlusion = prm.b;
    out.specular_f0 = mix(prm.a, transitionPrm.a, transitionFactor);

    // TODO: combine with sss params?
    out.sss_blend = out.sss_blend * prm.r;

    // Skin shaders use metalness for masking the fake SSS effect.
    if (per_material.has_vector[30].x == 1u) {
        out.metalness = 0.0;
    }

    let albedoColor = GetAlbedoColor(map1, uvSet, uvSet1, reflectionVector, colorSet5);
    var albedoRgb = GetAlbedoColorFinal(albedoColor);
    albedoRgb = mix(albedoRgb, transitionAlbedo, transitionFactor);
    out.albedo = vec4(albedoRgb, albedoColor.a);

    out.emission = GetEmissionColor(map1, uvSet);

    return out;
}

// TODO: use a struct to share code with fs_debug.
// the struct can hold the final base color, roughness, etc (principled)
// does modifying a struct in a function work so we can have ApplyMaterialTransition, ApplyRenderSettings, etc?
// consistent naming conventions with rust code?
@fragment
fn fs_main(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let map1 = in.map1.xy;
    let map1_dual = in.map1.zw;
    let uvSet = in.uv_set_uv_set1.xy;
    let uvSet1 = in.uv_set_uv_set1.zw;
    let uvSet2 = in.uv_set2_bake1.xy;
    let bake1 = in.uv_set2_bake1.zw;

    let colorSet1 = in.color_set1;
    let colorSet2 = in.color_set2_combined;
    let colorSet3 = in.color_set3;
    let colorSet4 = in.color_set4;
    let colorSet5 = in.color_set5;
    let colorSet6 = in.color_set6;
    let colorSet7 = in.color_set7;

    // TODO: Move this to GetPbrParams?
    // Normal code ported from in game.
    // This is similar to mikktspace but normalization happens in the fragment shader.
    let normal = normalize(in.normal.xyz);
    let tangent = normalize(in.tangent.xyz);
    var bitangent = GetBitangent(normal, tangent, in.tangent.w);

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);
    
    // TODO: Is it just the metal material that uses the fragment normal?
    var reflectionVector = reflect(viewVector, normal);
    reflectionVector.y = reflectionVector.y * -1.0;

    let params = GetPbrParams(in, viewVector, reflectionVector);

    var fragmentNormal = normal;
    if (per_material.has_texture[4].x == 1u) {
        fragmentNormal = GetBumpMapNormal(normal, tangent, bitangent, params.nor);
    }

    // TODO: Is this correct?
    bitangent = GetBitangent(fragmentNormal, tangent, in.tangent.w);

    // TODO: Investigate lighting for double sided materials with culling disabled.
    if (!is_front) {
        fragmentNormal = fragmentNormal * -1.0;
    }

    // TODO: Is it just the metal material that uses the fragment normal?
    reflectionVector = reflect(viewVector, fragmentNormal);
    reflectionVector.y = reflectionVector.y * -1.0;

    let chrLightDir = GetLight().direction.xyz;

    let halfAngle = normalize(chrLightDir + viewVector);
    let nDotV = max(dot(fragmentNormal, viewVector), 0.0);
    let nDotH = clamp(dot(fragmentNormal, halfAngle), 0.0, 1.0);
    let nDotL = dot(fragmentNormal, normalize(chrLightDir));


    var shadow = 1.0;
    if (render_settings.render_shadows.x == 1u && per_material.lighting_settings.z == 1u) {
        shadow = GetShadow(in.light_position);
    }

    var outAlpha = params.albedo.a * params.emission.a;
    if (per_material.has_vector[0].x == 1u) {
        outAlpha = max(params.albedo.a * params.emission.a, per_material.custom_vector[0].x);
    }
    if (per_material.shader_settings.x == 1u && outAlpha < 0.5) {
        discard;
    }

    let specularLod = RoughnessToLod(params.roughness);
    let specularIbl = textureSampleLevel(texture7, sampler7, reflectionVector, specularLod).rgb;

    let diffusePass = DiffuseTerm(bake1, params.albedo.rgb, nDotL, in.sh_lighting.rgb, params.ambient_occlusion, params.sss_color, params.sss_smooth_factor, params.sss_blend, shadow, colorSet2);

    // TODO: move this into GetPbrParams?
    let specularF0 = GetF0FromSpecular(params.specular_f0);

    let specularReflectionF0 = vec3(specularF0);
    // Metals use albedo instead of the specular color/tint.
    let kSpecular = mix(specularReflectionF0, params.albedo.rgb, params.metalness);
    // TODO: Not all shaders use nor.a as a cavity map (check if has texture).
    // TODO: Include ambient occlusion in specular?
    // TODO: Does cavity occlude ambient specular?
    let kDirect = kSpecular * shadow * params.nor.a;
    // TODO: Is this correct for masking environment reflections?
    let kIndirect = FresnelSchlick(nDotV, kSpecular) * params.nor.a * 0.5; // TODO: Why is 0.5 needed here?
    let specularPass = SpecularTerm(in.tangent, bitangent, nDotH, max(nDotL, 0.0), nDotV, halfAngle, params.roughness, specularIbl, kDirect, kIndirect);

    var outColor = vec3(0.0, 0.0, 0.0);
    if (render_settings.render_diffuse.x == 1u) {
        let kDiffuse = max(vec3(1.0 - params.metalness), vec3(0.0));
        outColor = outColor + (diffusePass * kDiffuse) / 3.14159;
    }

    // Assume materials without PRM omit the specular code entirely.
    if (render_settings.render_specular.x == 1u) {
        outColor = outColor + specularPass * params.ambient_occlusion;
    }

    if (render_settings.render_emission.x == 1u) {
        // TODO: Emission is weakened somehow?
        outColor = outColor + EmissionTerm(params.emission) * 0.5;
    }

    // TODO: What affects rim lighting intensity?
    if (render_settings.render_rim_lighting.x == 1u) {
        outColor = GetRimBlend(outColor, params.albedo.rgb, nDotV, max(nDotL, 0.0), shadow * params.nor.a, in.sh_lighting.rgb);
    }

    // TODO: Check all channels?
    if (per_material.has_vector[8].x == 1u) {
        outColor = outColor * per_material.custom_vector[8].rgb;
        outAlpha = outAlpha * per_material.custom_vector[8].a;
    }

    if (per_material.has_color_set1234.x == 1u && render_settings.render_vertex_color.x == 1u) {
        outColor = outColor * colorSet1.rgb;
        outAlpha = outAlpha * colorSet1.a;
    }

    if (per_material.has_color_set1234.z == 1u && render_settings.render_vertex_color.x == 1u) {
        outColor = outColor * colorSet3.rgb;
        outAlpha = outAlpha * colorSet3.a;
    }


    // TODO: Use FinalColorGain and FinalColorOffset like the in game shaders.
    // TODO: Finish analyzing the in game code for fog.
    // TODO: Move this to the vertex shader?
    // Linearly interpolate between the near and far threshold.
    // TODO: How to account for the offset in CustomVector13.w?
    // TODO: Should this be the frag pos or the object space position?
    // TODO: CustomVector9.x for fog intensity from material.
    let depth = -in.position.z;
    var fogIntensity = smoothstep(depth, stage_uniforms.scene_attributes.custom_vector[13].x, stage_uniforms.scene_attributes.custom_vector[13].y);
    let fogNear = stage_uniforms.scene_attributes.custom_vector[13].x;
    let fogFar = stage_uniforms.scene_attributes.custom_vector[13].y;
    fogIntensity = clamp((depth - fogNear) / fogFar, 0.0, 1.0);

    outColor = mix(outColor, stage_uniforms.scene_attributes.custom_vector[1].rgb, fogIntensity * 0.1);

    if (per_material.has_float[19].x == 1u) {
        outAlpha = GetAngleFade(nDotV, per_material.custom_float[19].x, specularF0);
    }

    // Premultiplied alpha.
    if (per_material.shader_settings.y == 1u) {
        outColor = outColor * outAlpha;
    }

    // Alpha override.
    if (per_material.has_boolean[2].x == 1u && per_material.custom_boolean[2].x == 1u) {
        outAlpha = 0.0;
    }

    return vec4(outColor, outAlpha);
}