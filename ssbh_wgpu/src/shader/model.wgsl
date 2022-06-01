struct CameraTransforms {
    model_view_matrix: mat4x4<f32>;
    mvp_matrix: mat4x4<f32>;
    camera_pos: vec4<f32>;
};

struct LightTransforms {
    light_transform: mat4x4<f32>;
};

// TODO: How to handle alignment?
struct RenderSettings {
    debug_mode: vec4<i32>;
    render_diffuse: vec4<f32>;
    render_specular: vec4<f32>;
    render_emission: vec4<f32>;
    render_rim_lighting: vec4<f32>;
    render_shadows: vec4<f32>;
};

// TODO: Bind groups should be ordered by how frequently they change for performance.
// group0 = PerFrame
// group1 = PerMaterial
// ... 
[[group(0), binding(0)]]
var<uniform> camera: CameraTransforms;

[[group(0), binding(1)]]
var texture_shadow: texture_2d<f32>;
[[group(0), binding(2)]]
var sampler_shadow: sampler;
// TODO: Specify that this is just the main character light?
// TODO: Does Smash Ultimate support shadow casting from multiple lights?
[[group(0), binding(3)]]
var<uniform> light: LightTransforms;

[[group(0), binding(4)]]
var<uniform> render_settings: RenderSettings;

// TODO: Is there a better way of organizing this?
// TODO: How many textures can we have?
[[group(1), binding(0)]]
var texture0: texture_2d<f32>;
[[group(1), binding(1)]]
var sampler0: sampler;

[[group(1), binding(2)]]
var texture1: texture_2d<f32>;
[[group(1), binding(3)]]
var sampler1: sampler;

[[group(1), binding(4)]]
var texture2: texture_cube<f32>;
[[group(1), binding(5)]]
var sampler2: sampler;

[[group(1), binding(6)]]
var texture3: texture_2d<f32>;
[[group(1), binding(7)]]
var sampler3: sampler;

[[group(1), binding(8)]]
var texture4: texture_2d<f32>;
[[group(1), binding(9)]]
var sampler4: sampler;

[[group(1), binding(10)]]
var texture5: texture_2d<f32>;
[[group(1), binding(11)]]
var sampler5: sampler;

[[group(1), binding(12)]]
var texture6: texture_2d<f32>;
[[group(1), binding(13)]]
var sampler6: sampler;

[[group(1), binding(14)]]
var texture7: texture_cube<f32>;
[[group(1), binding(15)]]
var sampler7: sampler;

[[group(1), binding(16)]]
var texture8: texture_cube<f32>;
[[group(1), binding(17)]]
var sampler8: sampler;

[[group(1), binding(18)]]
var texture9: texture_2d<f32>;
[[group(1), binding(19)]]
var sampler9: sampler;

[[group(1), binding(20)]]
var texture10: texture_2d<f32>;
[[group(1), binding(21)]]
var sampler10: sampler;

[[group(1), binding(22)]]
var texture11: texture_2d<f32>;
[[group(1), binding(23)]]
var sampler11: sampler;

[[group(1), binding(24)]]
var texture12: texture_2d<f32>;
[[group(1), binding(25)]]
var sampler12: sampler;

[[group(1), binding(26)]]
var texture13: texture_2d<f32>;
[[group(1), binding(27)]]
var sampler13: sampler;

[[group(1), binding(28)]]
var texture14: texture_2d<f32>;
[[group(1), binding(29)]]
var sampler14: sampler;

// Align everything to 16 bytes to avoid alignment issues.
// Smash Ultimate's shaders also use this alignment.
// TODO: Investigate std140/std430
// TODO: Does wgsl/wgpu require a specific layout/alignment?
struct MaterialUniforms {
    custom_vector: array<vec4<f32>, 64>;
    // TODO: Place the has_ values in an unused vector component?
    custom_boolean: array<vec4<f32>, 20>;
    custom_float: array<vec4<f32>, 20>;
    has_float: array<vec4<f32>, 20>;
    has_texture: array<vec4<f32>, 19>;
    has_vector: array<vec4<f32>, 64>;
};

[[group(1), binding(30)]]
var<uniform> uniforms: MaterialUniforms;

// TODO: Store light transform here as well?
// TODO: How to store lights?
struct StageUniforms {
    chr_light_dir: vec4<f32>;
    custom_boolean: array<vec4<f32>, 20>;
    custom_vector: array<vec4<f32>, 64>;
    custom_float: array<vec4<f32>, 20>;
};

// TODO: Move these to the first group since they don't change much?
[[group(2), binding(0)]]
var<uniform> stage_uniforms: StageUniforms;

struct VertexInput0 {
    [[location(0)]] position0: vec4<f32>;
    [[location(1)]] normal0: vec4<f32>;
    [[location(2)]] tangent0: vec4<f32>;
};

// We can safely assume 16 available locations.
// Pack attributes to avoid going over the attribute limit.
struct VertexInput1 {
    [[location(3)]] map1_uvset: vec4<f32>;
    [[location(4)]] uv_set1_uv_set2: vec4<f32>;
    [[location(5)]] bake1: vec4<f32>;
    [[location(6)]] color_set1345_packed: vec4<u32>;
    [[location(7)]] color_set2_packed: vec4<u32>;
    [[location(8)]] color_set67_packed: vec4<u32>;
};

// TODO: This will need to be reworked at some point.
struct VertexOutput {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec3<f32>;
    [[location(1)]] normal: vec3<f32>;
    [[location(2)]] tangent: vec4<f32>;
    [[location(3)]] map1_uvset: vec4<f32>;
    [[location(4)]] uv_set1_uv_set2: vec4<f32>;
    [[location(5)]] bake1: vec2<f32>;
    [[location(6)]] color_set1: vec4<f32>;
    [[location(7)]] color_set2_combined: vec4<f32>;
    [[location(8)]] color_set3: vec4<f32>;
    [[location(9)]] color_set4: vec4<f32>;
    [[location(10)]] color_set5: vec4<f32>;
    [[location(11)]] color_set6: vec4<f32>;
    [[location(12)]] color_set7: vec4<f32>;
    [[location(13)]] light_position: vec4<f32>;
};

fn Blend(a: vec3<f32>, b: vec4<f32>) -> vec3<f32> {
    // CustomBoolean11 toggles additive vs alpha blending.
    if (uniforms.custom_boolean[11].x != 0.0) {
        return a.rgb + b.rgb * b.a;
    } else {
        return mix(a.rgb, b.rgb, b.a);
    }
}

fn TransformUv(uv: vec2<f32>, transform: vec4<f32>) -> vec2<f32>
{
    let translate = vec2<f32>(-1.0 * transform.z, transform.w);

    // TODO: Does this affect all layers?
    // if (CustomBoolean5 == 1 || CustomBoolean6 == 1)
    //     translate *= currentFrame / 60.0;

    let scale = transform.xy;
    var result = (uv + translate) * scale;

    // dUdV Map.
    // Remap [0,1] to [-1,1].
    // let textureOffset = textureSample(texture4, sampler4, uv * 2.0).xy * 2.0 - 1.0;
    // result = result + textureOffset * uniforms.custom_float[4].x;

    return result;
}

// TODO: Rework texture blending to match the in game behavior.
// The game usually uses white for missing required textures.
// We use a single shader for all possible shaders.
// This requires a conditional check for each texture to render correctly.
// TODO: Ignore textures not used by the shader?
// This could probably be loaded from Rust as has_attribute & requires_attribute.
fn GetEmissionColor(uv1: vec2<f32>, uv2: vec2<f32>) -> vec4<f32> {
    var emissionColor = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    
    if (uniforms.has_texture[5].x == 1.0) {
        emissionColor = textureSample(texture5, sampler5, uv1);
    }

    if (uniforms.has_texture[14].x == 1.0) {
        let emission2Color = textureSample(texture14, sampler14, uv2);
        return vec4<f32>(Blend(emissionColor.rgb, emission2Color), emissionColor.a);
    }

    return emissionColor;
}

fn GetAlbedoColor(uv1: vec2<f32>, uv2: vec2<f32>, uv3: vec2<f32>, R: vec3<f32>, colorSet5: vec4<f32>) -> vec4<f32>
{
    let uvLayer1 = uv1;
    let uvLayer2 = uv2;
    let uvLayer3 = uv3;

    var outRgb = vec3<f32>(0.0);
    var outAlpha = 1.0;

    // TODO: Do additional layers affect alpha?
    if (uniforms.has_texture[0].x == 1.0) {
        let albedoColor = textureSample(texture0, sampler0, uvLayer1);
        outRgb = Blend(outRgb, albedoColor);
        outAlpha = albedoColor.a;
    }

    // TODO: colorSet5.w is used to blend between the two col map layers.
    // TODO: Add has_color_set.
    if (uniforms.has_texture[1].x == 1.0) {
        let albedoColor2 = textureSample(texture1, sampler1, uvLayer2);
        outRgb = Blend(outRgb, albedoColor2 * vec4<f32>(1.0, 1.0, 1.0, 1.0));
    }

    // Materials won't have col and diffuse cube maps.
    if (uniforms.has_texture[8].x == 1.0) {
        outRgb = textureSample(texture8, sampler8, R).rgb;
    }

    if (uniforms.has_texture[10].x == 1.0) {
        outRgb = Blend(outRgb, textureSample(texture10, sampler10, uvLayer1));
    }
    // TODO: Is the blending always additive?
    if (uniforms.has_texture[11].x == 1.0) {
        outRgb = Blend(outRgb, textureSample(texture11, sampler11, uvLayer2));
    }
    if (uniforms.has_texture[12].x == 1.0) {
        outRgb = outRgb + textureSample(texture12, sampler12, uvLayer3).rgb;
    }

    return vec4<f32>(outRgb, outAlpha);
}

fn GetAlbedoColorFinal(albedoColor: vec4<f32>) -> vec3<f32>
{    
    var albedoColorFinal = albedoColor.rgb;

    // Color multiplier param.
    if (uniforms.has_vector[13].x == 1.0) {
        albedoColorFinal = albedoColorFinal * uniforms.custom_vector[13].rgb;
    }

    // TODO: Wiifit stage model color.
    // if (hasCustomVector44 == 1) {
    //     albedoColorFinal = uniforms.custom_vector[44].rgb + uniforms.custom_vector[45].rgb;
    // }

    return albedoColorFinal;
}


fn GetBitangent(normal: vec3<f32>, tangent: vec3<f32>, tangentSign: f32) -> vec3<f32>
{
    // Flip after normalization to avoid issues with tangentSign being 0.0.
    // Flip after normalization to avoid issues with tangentSign being 0.0.
    // Smash Ultimate requires Tangent0.W to be flipped.
    // Smash Ultimate requires Tangent0.W to be flipped.
    return normalize(cross(normal.xyz, tangent.xyz)) * tangentSign * -1.0;
}
    
fn GetBumpMapNormal(normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>, norColor: vec4<f32>) -> vec3<f32>
{
    // Remap the normal map to the correct range.
    // Remap the normal map to the correct range.
    let x = 2.0 * norColor.x - 1.0;
    let y = 2.0 * norColor.y - 1.0;
    
    // Calculate z based on the fact that x*x + y*y + z*z = 1.
    // Calculate z based on the fact that x*x + y*y + z*z = 1.
    // Clamp to prevent z being 0.0.
    // Clamp to prevent z being 0.0.
    let z = sqrt(max(1.0 - (x * x) + (y * y), 0.001));
    
    let normalMapNormal = vec3<f32>(x, y, z);
    
    let tbnMatrix = mat3x3<f32>(tangent, bitangent, normal);
    
    let newNormal = tbnMatrix * normalMapNormal;
    return normalize(newNormal);
}

// Schlick fresnel approximation.
fn FresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32>
{
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
} 

// Ultimate shaders use a schlick geometry masking term.
// http://cwyman.org/code/dxrTutors/tutors/Tutor14/tutorial14.md.html
fn SchlickMaskingTerm(nDotL: f32, nDotV: f32, a2: f32) -> f32
{
    // TODO: Double check this masking term.
    let k = a2 * 0.5;
    let gV = nDotV / (nDotV * (1.0 - k) + k);
    let gL = nDotL / (nDotL * (1.0 - k) + k);
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
    let specular = a2 / (PI * denominator * denominator);
    let shadowing = SchlickMaskingTerm(nDotL, nDotV, a2);
    // TODO: double check the denominator
    return specular * shadowing / 3.141519;
}

// A very similar BRDF as used for GGX.
fn GgxAnisotropic(nDotH: f32, h: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>, roughness: f32, anisotropy: f32) -> f32
{
    // TODO: How much of this is shared with GGX?
    // Clamp to 0.01 to prevent divide by 0.
    let roughnessX = max(roughness * anisotropy, 0.01);
    let roughnessY = max(roughness / anisotropy, 0.01);

    let roughnessX4 = pow(roughnessX, 4.0);
    let roughnessY4 = pow(roughnessY, 4.0);

    let xDotH = dot(bitangent, h);
    let xTerm = (xDotH * xDotH) / roughnessX4;

    let yDotH = dot(tangent, h);
    let yTerm = (yDotH * yDotH) / roughnessY4;

    // TODO: Check this section of code.
    let nDotH2 = nDotH * nDotH;
    let denominator = xTerm + yTerm + nDotH2;

    // TODO: Is there a geometry term for anisotropic?
    let normalization = (3.14159 * roughnessX * roughnessY);
    return 1.0 / (normalization * denominator * denominator);
}

fn DiffuseTerm(
    bake1: vec2<f32>, 
    albedo: vec3<f32>, 
    nDotL: f32, 
    ambientLight: vec3<f32>, 
    ao: vec3<f32>, 
    sssBlend: f32, 
    shadow: f32) -> vec3<f32>
{
    // TODO: This can be cleaned up.
    var directShading = albedo * max(nDotL, 0.0);

    // TODO: nDotL is a vertex attribute for skin shading.

    // Diffuse shading is remapped to be softer.
    // Multiplying be a constant and clamping affects the "smoothness".
    var nDotLSkin = nDotL * uniforms.custom_vector[30].y;
    nDotLSkin = clamp(nDotLSkin * 0.5 + 0.5, 0.0, 1.0);
    let skinShading = uniforms.custom_vector[11].rgb * sssBlend * nDotLSkin;

    // TODO: How many PI terms are there?
    // TODO: Skin shading looks correct without the PI term?
    directShading = mix(directShading / 3.14159, skinShading, sssBlend);

    var directLight = stage_uniforms.custom_vector[0].rgb * stage_uniforms.custom_float[0].x * directShading;
    var ambientTerm = (ambientLight * ao);

    if (uniforms.has_texture[9].x == 1.0) {
        let bakedLitColor = textureSample(texture9, sampler9, bake1).rgba;
        directLight = directLight * bakedLitColor.a;
        // Baked lighting maps are not affected by ambient occlusion.
        ambientTerm = ambientTerm + (bakedLitColor.rgb * 8.0);
    }

    // Assume the mix factor is 0.0 if the material doesn't have CustomVector11.
    ambientTerm = ambientTerm * mix(albedo, uniforms.custom_vector[11].rgb, sssBlend);

    let result = directLight * shadow + ambientTerm;

    // Baked stage lighting.
    //if (renderVertexColor == 1 && hasColorSet2 == 1)
    //    result *= colorSet2.rgb;

    return result;
}

// Create a rotation matrix to rotate around an arbitrary axis.
//http://www.neilmendoza.com/glsl-rotation-about-an-arbitrary-axis/
// fn rotationMatrix(axis: vec3<f32>, angle: f32) -> mat4x4<f32>
// {
//     let axis = normalize(axis);
//     let s = sin(angle);
//     let c = cos(angle);
//     let oc = 1.0 - c;

//     return mat4x4<f32>(oc * axis.x * axis.x + c, oc * axis.x * axis.y - axis.z * s,  oc * axis.z * axis.x + axis.y * s, 0.0, oc * axis.x * axis.y + axis.z * s, oc * axis.y * axis.y + c, oc * axis.y * axis.z - axis.x * s,  0.0, oc * axis.z * axis.x - axis.y * s,  oc * axis.y * axis.z + axis.x * s,  oc * axis.z * axis.z + c, 0.0, 0.0, 0.0, 0.0, 1.0);
// }

// TODO: Make bitangent and argument?
fn SpecularBrdf(tangent: vec4<f32>, nDotH: f32, nDotL: f32, nDotV: f32, halfAngle: vec3<f32>, normal: vec3<f32>, roughness: f32, anisotropicRotation: f32) -> f32
{
    let angle = anisotropicRotation * 3.14159;
    //let tangentMatrix = rotationMatrix(normal, angle);
    //let rotatedTangent = mat3x3<f32>(tangentMatrix) * tangent.xyz;
    // TODO: How is the rotation calculated for tangents and bitangents?
    let bitangent = GetBitangent(normal, tangent.xyz, tangent.w);
    // The two BRDFs look very different so don't just use anisotropic for everything.
    if (uniforms.has_float[10].x == 1.0) {
        return GgxAnisotropic(nDotH, halfAngle, tangent.xyz, bitangent, roughness, uniforms.custom_float[10].x);
    } else {
        return Ggx(nDotH, nDotL, nDotV, roughness);
    }
}

fn SpecularTerm(tangent: vec4<f32>, nDotH: f32, nDotL: f32, nDotV: f32, halfAngle: vec3<f32>, normal: vec3<f32>, roughness: f32, 
    specularIbl: vec3<f32>, metalness: f32, anisotropicRotation: f32,
    shadow: f32) -> vec3<f32>
{
    var directSpecular = vec3<f32>(4.0);
    directSpecular = directSpecular * SpecularBrdf(tangent, nDotH, nDotL, nDotV, halfAngle, normal, roughness, anisotropicRotation);
    directSpecular = directSpecular * 1.0;
    let indirectSpecular = specularIbl;
    // TODO: Why is the indirect specular off by a factor of 0.5?
    let specularTerm = (directSpecular * shadow) + (indirectSpecular * 0.5);

    return specularTerm;
}

fn EmissionTerm(emissionColor: vec4<f32>) -> vec3<f32>
{
    var result = emissionColor.rgb;
    if (uniforms.has_vector[3].x == 1.0) {
        result = result * uniforms.custom_vector[3].rgb;
    }

    return result;
}

fn GetF0FromIor(ior: f32) -> f32
{
    return pow((1.0 - ior) / (1.0 + ior), 2.0);
}

fn Luminance(rgb: vec3<f32>) -> f32
{
    let W = vec3<f32>(0.2125, 0.7154, 0.0721);
    return dot(rgb, W);
}

fn GetSpecularWeight(f0: f32, diffusePass: vec3<f32>, metalness: f32, nDotV: f32, roughness: f32) -> vec3<f32>
{
    // Metals use albedo instead of the specular color/tint.
    let specularReflectionF0 = vec3<f32>(f0);
    let f0Final = mix(specularReflectionF0, diffusePass, metalness);
    return FresnelSchlick(nDotV, f0Final);
}

// TODO: Is this just a regular lighting term?
// TODO: Does this depend on the light direction and intensity?
fn GetRimBlend(baseColor: vec3<f32>, diffusePass: vec3<f32>, nDotV: f32, nDotL: f32, occlusion: f32, vertexAmbient: vec3<f32>) -> vec3<f32>
{
    var rimColor = uniforms.custom_vector[14].rgb * stage_uniforms.custom_vector[8].rgb;

    // TODO: How is the overall intensity controlled?
    // Hardcoded shader constant.
    let rimIntensity = 0.2125999927520752;
    // rimColor *= rimIntensity;

    // TODO: There some sort of directional lighting that controls the intensity of this effect.
    // This appears to be lighting done in the vertex shader.
    rimColor = rimColor * vertexAmbient;

    // TODO: Black edges for large blend values?
    // Edge tint.
    rimColor = rimColor * clamp(mix(vec3<f32>(1.0), diffusePass, uniforms.custom_float[8].x), vec3<f32>(0.0), vec3<f32>(1.0));

    let fresnel = pow(1.0 - nDotV, 5.0);
    var rimBlend = fresnel * stage_uniforms.custom_vector[8].w * uniforms.custom_vector[14].w * 0.6;
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
    let facingRatio = FresnelSchlick(nDotV, vec3<f32>(f0AngleFade)).x;
    return max(facingRatio, specularf0);
}

fn GetF0FromSpecular(specular: f32) -> f32
{
    // Specular gets remapped from [0.0,1.0] to [0.0,0.2].
    // The value is 0.16*0.2 = 0.032 if the PRM alpha is ignored.
    if (uniforms.custom_boolean[1].x == 0.0) {
        return 0.16 * 0.2;
    }

    return specular * 0.2;
}

// Shadow mapping.
fn GetShadow(light_position: vec4<f32>) -> f32
{
    // compensate for the Y-flip difference between the NDC and texture coordinates
    let flipCorrection = vec2<f32>(0.5, -0.5);
    // compute texture coordinates for shadow lookup
    let projCorrection = 1.0 / light_position.w;
    let light_local = light_position.xy * flipCorrection * projCorrection + vec2<f32>(0.5, 0.5);

    // TODO: This assumes depth is in the range 0.0 to 1.0 in the texture.
    let currentDepth = light_position.z * projCorrection;

    // Translated variance shadow mapping from in game.
    let m1 = textureSample(texture_shadow, sampler_shadow, light_local).r;
    let m2 = textureSample(texture_shadow, sampler_shadow, light_local).g;
    let sigma2 = clamp(m2 - m1*m1 + 0.0001, 0.0, 1.0);
    let tDif = max(currentDepth - m1, 0.0);
    // Approximate Pr(x >= t) using one of Chebychev's inqequalities.
    var shadow = sigma2 / (sigma2 + tDif*tDif);
    // TODO: Why is there a pow(shadow, 4.0) in game?
    shadow = pow(shadow, 4.0);
    return shadow;
}

[[stage(vertex)]]
fn vs_main(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> VertexOutput {
    var out: VertexOutput;
    out.position = buffer0.position0.xyz;
    out.clip_position = camera.mvp_matrix * vec4<f32>(buffer0.position0.xyz, 1.0);
    out.normal = buffer0.normal0.xyz;
    out.tangent = buffer0.tangent0;
    
    let colorSet1 = unpack4x8unorm(buffer1.color_set1345_packed.x);
    let colorSet3 = unpack4x8unorm(buffer1.color_set1345_packed.y);
    let colorSet4 = unpack4x8unorm(buffer1.color_set1345_packed.z);
    let colorSet5 = unpack4x8unorm(buffer1.color_set1345_packed.w);
    let colorSet2 = unpack4x8unorm(buffer1.color_set2_packed.x);
    let colorSet2_11 = unpack4x8unorm(buffer1.color_set2_packed.y);
    let colorSet2_2 = unpack4x8unorm(buffer1.color_set2_packed.z);
    let colorSet2_3 = unpack4x8unorm(buffer1.color_set2_packed.w);
    let colorSet6 = unpack4x8unorm(buffer1.color_set67_packed.x);
    let colorSet7 = unpack4x8unorm(buffer1.color_set67_packed.y);


    // TODO: Also apply transforms to the debug shader?
    var uvTransform1 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[6].x == 1.0) {
        uvTransform1 = uniforms.custom_vector[6];
    }

    var uvTransform2 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[31].x == 1.0) {
        uvTransform2 = uniforms.custom_vector[31];
    }

    var uvTransform3 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[32].x == 1.0) {
        uvTransform3 = uniforms.custom_vector[32];
    }

    let map1 = TransformUv(buffer1.map1_uvset.xy, uvTransform1);
    let uvSet = TransformUv(buffer1.map1_uvset.zw, uvTransform2);
    let uvSet1 = TransformUv(buffer1.uv_set1_uv_set2.xy, uvTransform3);
    // TODO: Transform for uvSet2?
    let uvSet2 = TransformUv(buffer1.uv_set1_uv_set2.xy, uvTransform3);

    out.map1_uvset = vec4<f32>(map1, uvSet);
    out.uv_set1_uv_set2 = vec4<f32>(uvSet1, uvSet2);
    out.bake1 = buffer1.bake1.xy;
    out.color_set1 = colorSet1;
    out.color_set2_combined = colorSet2; // TODO: colorSet2 is added together?
    out.color_set3 = colorSet3;
    out.color_set4 = colorSet4;
    out.color_set5 = colorSet5;
    out.color_set6 = colorSet6;
    out.color_set7 = colorSet7;

    out.light_position = light.light_transform * vec4<f32>(buffer0.position0.xyz, 1.0);
    return out;
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

[[stage(fragment)]]
fn fs_invalid_shader([[builtin(position)]] frag_pos: vec4<f32>) -> [[location(0)]] vec4<f32> {
    let checker = ScreenCheckerBoard(frag_pos.xy);
    return vec4<f32>(checker, 0.0, 0.0, 1.0);
}

[[stage(fragment)]]
fn fs_invalid_attributes([[builtin(position)]] frag_pos: vec4<f32>) -> [[location(0)]] vec4<f32> {
    let checker = ScreenCheckerBoard(frag_pos.xy);
    return vec4<f32>(checker, checker, 0.0, 1.0);
}


[[stage(fragment)]]
fn fs_debug(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let map1 = in.map1_uvset.xy;
    let uvSet = in.map1_uvset.zw;
    let uvSet1 = in.uv_set1_uv_set2.xy;
    let uvSet2 = in.uv_set1_uv_set2.zw;
    let bake1 = in.bake1.xy;

    let colorSet1 = in.color_set1;
    let colorSet2 = in.color_set2_combined;
    let colorSet3 = in.color_set3;
    let colorSet4 = in.color_set4;
    let colorSet5 = in.color_set5;
    let colorSet6 = in.color_set6;
    let colorSet7 = in.color_set7;

    let normal = normalize(in.normal.xyz);
    let tangent = normalize(in.tangent.xyz);

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);

    var reflectionVector = reflect(viewVector, normal);
    reflectionVector.y = reflectionVector.y * -1.0;

    switch (render_settings.debug_mode.x) {
        case 1: {
            let color = normalize(in.position.xyz) * 0.5 + 0.5;
            return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 2: {
            let color = normalize(in.normal.xyz) * 0.5 + 0.5;
            return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 3: {
            let color = normalize(in.tangent.xyz) * 0.5 + 0.5;
            return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 4: {
            return colorSet1;
        }
        case 5: {
            return colorSet2;
        }
        case 6: {
            return colorSet3;
        }
        case 7: {
            return colorSet4;
        }
        case 8: {
            return colorSet5;
        }
        case 9: {
            return colorSet6;
        }
        case 10: {
            return colorSet7;
        }
        case 11: {      
            return textureSample(texture0, sampler0, map1);
        }
        case 12: {      
            return textureSample(texture1, sampler1, uvSet);
        }
        case 13: {      
            return textureSample(texture2, sampler2, reflectionVector);
        }
        case 14: {      
            return textureSample(texture3, sampler3, bake1);
        }
        case 15: {      
            return textureSample(texture4, sampler4, map1);
        }
        case 16: {      
            return textureSample(texture5, sampler5, map1);
        }
        case 17: {      
            return textureSample(texture6, sampler6, map1);
        }
        case 18: {      
            return textureSample(texture7, sampler7, reflectionVector);
        }
        case 19: {      
            return textureSample(texture8, sampler8, reflectionVector);
        }
        case 20: {      
            return textureSample(texture9, sampler9, bake1);
        }
        case 21: {      
            return textureSample(texture10, sampler10, map1);
        }
        case 22: {      
            return textureSample(texture11, sampler11, uvSet);
        }
        case 23: {      
            return textureSample(texture12, sampler12, map1);
        }
        case 24: {      
            return textureSample(texture13, sampler13, map1);
        }
        case 25: {      
            return textureSample(texture14, sampler14, uvSet);
        }
        // case 26: {      
        //     return textureSample(texture16, sampler16, map1);
        // }
        case 27: {      
            return vec4<f32>(map1, 1.0, 1.0);
        }
        case 28: {      
            return vec4<f32>(bake1, 1.0, 1.0);
        }
        case 29: {      
            return vec4<f32>(uvSet, 1.0, 1.0);
        }
        case 30: {      
            return vec4<f32>(uvSet1, 1.0, 1.0);
        }
        case 31: {      
            return vec4<f32>(uvSet2, 1.0, 1.0);
        }
        default: { 
            return vec4<f32>(1.0);
        }
    }
    
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let map1 = in.map1_uvset.xy;
    let uvSet = in.map1_uvset.zw;
    let uvSet1 = in.uv_set1_uv_set2.xy;
    let uvSet2 = in.uv_set1_uv_set2.zw;
    let bake1 = in.bake1.xy;

    let colorSet1 = in.color_set1;
    let colorSet2 = in.color_set2_combined;
    let colorSet3 = in.color_set3;
    let colorSet4 = in.color_set4;
    let colorSet5 = in.color_set5;
    let colorSet6 = in.color_set6;
    let colorSet7 = in.color_set7;

    // TODO: Some of these textures are sampled more than once?
    var nor = vec4<f32>(0.5, 0.5, 1.0, 1.0);
    if (uniforms.has_texture[4].x == 1.0) {
        nor = textureSample(texture4, sampler4, map1);
    }

    var prm = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    if (uniforms.has_texture[6].x == 1.0) {
        prm = textureSample(texture6, sampler6, map1);
    }

    var metalness = prm.r;
    let roughness = prm.g;
    let ao = prm.b;
    let spec = prm.a;

    // Skin shaders use metalness for masking the fake SSS effect.
    if (uniforms.custom_vector[30].x > 0.0) {
        metalness = 0.0;
    }

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);

    let normal = normalize(in.normal.xyz);
    let tangent = normalize(in.tangent.xyz);
    let bitangent = normalize(cross(normal, tangent)) * in.tangent.w * -1.0;

    var fragmentNormal = normal;
    if (uniforms.has_texture[4].x == 1.0) {
        fragmentNormal = GetBumpMapNormal(normal, tangent, bitangent, nor);
    }

    var reflectionVector = reflect(viewVector, normal);
    reflectionVector.y = reflectionVector.y * -1.0;

    let chrLightDir = stage_uniforms.chr_light_dir.xyz;

    let halfAngle = normalize(chrLightDir + viewVector);
    let nDotV = max(dot(fragmentNormal, viewVector), 0.0);
    let nDotH = clamp(dot(fragmentNormal, halfAngle), 0.0, 1.0);
    let nDotL = dot(fragmentNormal, normalize(chrLightDir));

    let albedoColor = GetAlbedoColor(map1, uvSet, uvSet1, reflectionVector, colorSet5);
    let emissionColor = GetEmissionColor(map1, uvSet);

    let shadow = mix(1.0, GetShadow(in.light_position), render_settings.render_shadows.x);

    var outAlpha = max(albedoColor.a * emissionColor.a, uniforms.custom_vector[0].x);
    if (outAlpha < 0.5) {
        // TODO: This is disabled by some shaders.
        discard;
    }

    let sssBlend = prm.r * uniforms.custom_vector[30].x;

    // TODO: Apply multiplier param?
    var albedoColorFinal = GetAlbedoColorFinal(albedoColor);

    let specularF0 = GetF0FromSpecular(prm.a);

    let specularLod = RoughnessToLod(roughness);
    let specularIbl = textureSampleLevel(texture7, sampler7, reflectionVector, specularLod).rgb;

    // TODO: Vertex shader
    let shAmbientR = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.14186, 0.04903, -0.082, 1.11054));
    let shAmbientG = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.14717, 0.03699, -0.08283, 1.11036));
    let shAmbientB = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.1419, 0.04334, -0.08283, 1.11018));
    let shColor = vec3<f32>(shAmbientR, shAmbientG, shAmbientB);

    let diffusePass = DiffuseTerm(bake1, albedoColorFinal.rgb, nDotL, shColor, vec3<f32>(ao), sssBlend, shadow);

    let specularPass = SpecularTerm(in.tangent, nDotH, max(nDotL, 0.0), nDotV, halfAngle, fragmentNormal, roughness, specularIbl, metalness, prm.a, shadow);

    let kSpecular = GetSpecularWeight(specularF0, albedoColorFinal.rgb, metalness, nDotV, roughness);

    var kDiffuse = max((vec3<f32>(1.0) - kSpecular) * (1.0 - metalness), vec3<f32>(0.0));
    kDiffuse = max(vec3<f32>(1.0 - metalness), vec3<f32>(0.0));

    var outColor = vec3<f32>(0.0, 0.0, 0.0);
    outColor = outColor + (diffusePass * kDiffuse) / 3.14159 * render_settings.render_diffuse.x;
    outColor = outColor + specularPass * kSpecular * ao * render_settings.render_specular.x;
    // TODO: Emission is weakened somehow?
    outColor = outColor + EmissionTerm(emissionColor) * 0.5 * render_settings.render_emission.x;
    // TODO: What affects rim lighting intensity?
    let rimOcclusion = shadow * render_settings.render_rim_lighting.x;
    outColor = GetRimBlend(outColor, albedoColorFinal, nDotV, max(nDotL, 0.0), rimOcclusion, shColor);

    // TODO: Color sets?
    return vec4<f32>(outColor, outAlpha);
}