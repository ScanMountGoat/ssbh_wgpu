struct CameraTransforms {
    model_view_matrix: mat4x4<f32>;
    mvp_matrix: mat4x4<f32>;
    camera_pos: vec4<f32>;
    // width, height, scale, _
    screen_dimensions: vec4<f32>;
};

struct LightTransforms {
    light_transform: mat4x4<f32>;
};

// TODO: How to handle alignment?
struct RenderSettings {
    debug_mode: vec4<u32>;
    transition_material: vec4<u32>;
    transition_factor: vec4<f32>;
    render_diffuse: vec4<u32>;
    render_specular: vec4<u32>;
    render_emission: vec4<u32>;
    render_rim_lighting: vec4<u32>;
    render_shadows: vec4<u32>;
    render_bloom: vec4<u32>;
    render_rgba: vec4<f32>;
};

// TODO: Store light transform here as well?
// TODO: How to store lights?
struct StageUniforms {
    chr_light_dir: vec4<f32>;
    custom_boolean: array<vec4<f32>, 20>;
    custom_vector: array<vec4<f32>, 64>;
    custom_float: array<vec4<f32>, 20>;
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

[[group(0), binding(5)]]
var<uniform> stage_uniforms: StageUniforms;

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
    custom_boolean: array<vec4<u32>, 20>;
    custom_float: array<vec4<f32>, 20>;
    has_boolean: array<vec4<u32>, 20>;
    has_float: array<vec4<u32>, 20>;
    has_texture: array<vec4<u32>, 19>;
    has_vector: array<vec4<u32>, 64>;
    has_color_set1234: vec4<u32>;
    has_color_set567: vec4<u32>;
    is_discard: vec4<u32>;
};

[[group(1), binding(30)]]
var<uniform> uniforms: MaterialUniforms;

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
    [[location(6)]] color_set1: vec4<f32>;
    [[location(7)]] color_set2_combined: vec4<f32>;
    [[location(8)]] color_set3: vec4<f32>;
    [[location(9)]] color_set4: vec4<f32>;
    [[location(10)]] color_set5: vec4<f32>;
    [[location(11)]] color_set6: vec4<f32>;
    [[location(12)]] color_set7: vec4<f32>;
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

struct VertexOutputInvalid {
    [[builtin(position)]] clip_position: vec4<f32>;
    [[location(0)]] position: vec4<f32>;
};

fn Blend(a: vec3<f32>, b: vec4<f32>) -> vec3<f32> {
    // CustomBoolean11 toggles additive vs alpha blending.
    if (uniforms.custom_boolean[11].x == 1u) {
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
    
    if (uniforms.has_texture[5].x == 1u) {
        emissionColor = textureSample(texture5, sampler5, uv1);
    }

    if (uniforms.has_texture[14].x == 1u) {
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
    if (uniforms.has_texture[0].x == 1u) {
        let albedoColor = textureSample(texture0, sampler0, uvLayer1);
        outRgb = albedoColor.rgb;
        outAlpha = albedoColor.a;
    }

    // TODO: Refactor blend to take RGB and w separately?
    if (uniforms.has_texture[1].x == 1u) {
        let albedoColor2 = textureSample(texture1, sampler1, uvLayer2);
        if (uniforms.has_color_set567.x == 1u) {
            // colorSet5.w is used to blend between the two col map layers.
            outRgb = Blend(outRgb, albedoColor2 * vec4<f32>(1.0, 1.0, 1.0, colorSet5.w));
        } else {
            outRgb = Blend(outRgb, albedoColor2);
        }
    }

    // Materials won't have col and diffuse cube maps.
    if (uniforms.has_texture[8].x == 1u) {
        outRgb = textureSample(texture8, sampler8, R).rgb;
    }

    if (uniforms.has_texture[10].x == 1u) {
        outRgb = Blend(outRgb, textureSample(texture10, sampler10, uvLayer1));
    }
    // TODO: Is the blending always additive?
    if (uniforms.has_texture[11].x == 1u) {
        outRgb = Blend(outRgb, textureSample(texture11, sampler11, uvLayer2));
    }
    if (uniforms.has_texture[12].x == 1u) {
        outRgb = outRgb + textureSample(texture12, sampler12, uvLayer3).rgb;
    }

    return vec4<f32>(outRgb, outAlpha);
}

fn GetAlbedoColorFinal(albedoColor: vec4<f32>) -> vec3<f32>
{    
    var albedoColorFinal = albedoColor.rgb;

    // Color multiplier param.
    if (uniforms.has_vector[13].x == 1u) {
        albedoColorFinal = albedoColorFinal * uniforms.custom_vector[13].rgb;
    }

    // TODO: Wiifit stage model color.
    // if (hasCustomVector44 == 1u) {
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
    shadow: f32,
    custom_vector11: vec4<f32>,
    custom_vector30: vec4<f32>,
    colorSet2: vec4<f32>) -> vec3<f32>
{
    // TODO: This can be cleaned up.
    var directShading = albedo * max(nDotL, 0.0);

    // TODO: nDotL is a vertex attribute for skin shading.

    // Diffuse shading is remapped to be softer.
    // Multiplying be a constant and clamping affects the "smoothness".
    var nDotLSkin = nDotL * custom_vector30.y;
    nDotLSkin = clamp(nDotLSkin * 0.5 + 0.5, 0.0, 1.0);
    // TODO: Why is there an extra * sssBlend term here?
    let skinShading = custom_vector11.rgb * sssBlend * nDotLSkin;

    // TODO: How many PI terms are there?
    // TODO: Skin shading looks correct without the PI term?
    directShading = mix(directShading / 3.14159, skinShading, sssBlend);

    var directLight = stage_uniforms.custom_vector[0].rgb * stage_uniforms.custom_float[0].x * directShading;
    var ambientTerm = (ambientLight * ao);

    if (uniforms.has_texture[9].x == 1u) {
        let bakedLitColor = textureSample(texture9, sampler9, bake1).rgba;
        directLight = directLight * bakedLitColor.a;
        // Baked lighting maps are not affected by ambient occlusion.
        ambientTerm = ambientTerm + (bakedLitColor.rgb * 8.0);
    }

    // Assume the mix factor is 0.0 if the material doesn't have CustomVector11.
    ambientTerm = ambientTerm * mix(albedo, custom_vector11.rgb, sssBlend);

    var result = directLight * shadow + ambientTerm;

    // Baked stage lighting.
    if (uniforms.has_color_set1234.y == 1u) {
       result = result * colorSet2.rgb;
    }

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
    if (uniforms.has_float[10].x == 1u) {
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
    if (uniforms.has_boolean[3].x == 1u && uniforms.custom_boolean[3].x == 0u) {
        directSpecular = vec3<f32>(0.0);
    }

    var indirectSpecular = specularIbl;
    if (uniforms.has_boolean[4].x == 1u && uniforms.custom_boolean[4].x == 0u) {
        directSpecular = vec3<f32>(0.0);
    }

    // TODO: Why is the indirect specular off by a factor of 0.5?
    let specularTerm = (directSpecular * shadow) + (indirectSpecular * 0.5);

    return specularTerm;
}

fn EmissionTerm(emissionColor: vec4<f32>) -> vec3<f32>
{
    var result = emissionColor.rgb;
    if (uniforms.has_vector[3].x == 1u) {
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
    if (uniforms.has_boolean[1].x == 1u && uniforms.custom_boolean[1].x == 0u) {
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
    
    // Apply color scaling since the attribute is stored as four bytes.
    // This allows values greater than the normal 0.0 to 1.0 range.
    let colorSet1 = buffer1.color_set1 * 2.0;
    let colorSet2 = (buffer1.color_set2_combined * buffer1.color_set2_combined) * 7.0;
    let colorSet3 = buffer1.color_set3 * 2.0;
    let colorSet4 = buffer1.color_set4 * 2.0;
    let colorSet5 = buffer1.color_set5 * 3.0;
    let colorSet6 = buffer1.color_set6;
    let colorSet7 = buffer1.color_set7;

    // TODO: Also apply transforms to the debug shader?
    var uvTransform1 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[6].x == 1u) {
        uvTransform1 = uniforms.custom_vector[6];
    }

    var uvTransform2 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[31].x == 1u) {
        uvTransform2 = uniforms.custom_vector[31];
    }

    var uvTransform3 = vec4<f32>(1.0, 1.0, 0.0, 0.0);
    if (uniforms.has_vector[32].x == 1u) {
        uvTransform3 = uniforms.custom_vector[32];
    }

    var map1 = TransformUv(buffer1.map1_uvset.xy, uvTransform1);
    // Sprite sheet params.
    // Perform this in the fragment shader to avoid effecting debug modes.
    if (uniforms.custom_boolean[9].x == 1u) {
        map1 = map1 / uniforms.custom_vector[18].xy;
    }

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

[[stage(vertex)]]
fn vs_depth(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> [[builtin(position)]] vec4<f32> {
    return light.light_transform * vec4<f32>(buffer0.position0.xyz, 1.0);
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

[[stage(vertex)]]
fn vs_main_invalid(
    buffer0: VertexInput0,
    buffer1: VertexInput1
) -> VertexOutputInvalid {
    var out: VertexOutputInvalid;
    out.clip_position = camera.mvp_matrix * vec4<f32>(buffer0.position0.xyz, 1.0);
    out.position = out.clip_position;

    return out;
}

[[stage(fragment)]]
fn fs_invalid_shader(in: VertexOutputInvalid) -> [[location(0)]] vec4<f32> {
    let position_clip = (in.position.xy / in.position.w) * 0.5 + 0.5;
    // Account for screen dimensions and scale.
    let checker = ScreenCheckerBoard(position_clip * camera.screen_dimensions.xy * camera.screen_dimensions.z);
    return vec4<f32>(checker, 0.0, 0.0, 1.0);
}

[[stage(fragment)]]
fn fs_invalid_attributes(in: VertexOutputInvalid) -> [[location(0)]] vec4<f32> {
    let position_clip = (in.position.xy / in.position.w) * 0.5 + 0.5;
    // Account for screen dimensions and scale.
    let checker = ScreenCheckerBoard(position_clip * camera.screen_dimensions.xy * camera.screen_dimensions.z);
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
    let bitangent = normalize(cross(normal, tangent)) * in.tangent.w * -1.0;

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);

    var reflectionVector = reflect(viewVector, normal);
    reflectionVector.y = reflectionVector.y * -1.0;

    // TODO: Apply normal maps and convert to view space.
    var fragmentNormal = normal;
    if (uniforms.has_texture[4].x == 1u) {
        let nor = textureSample(texture4, sampler4, map1);
        fragmentNormal = GetBumpMapNormal(normal, tangent, bitangent, nor);
    }

    var prm = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    if (uniforms.has_texture[6].x == 1u) {
        prm = textureSample(texture6, sampler6, map1);
    }

    // Move fake subsurface color into GetAlbedoColorFinal?
    let sssBlend = prm.r * uniforms.custom_vector[30].x;
    let albedoColor = GetAlbedoColor(map1, uvSet, uvSet1, reflectionVector, colorSet5);
    var albedoColorFinal = GetAlbedoColorFinal(albedoColor);
    albedoColorFinal = mix(albedoColorFinal, uniforms.custom_vector[11].rgb, sssBlend);

    // TODO: Some of these render modes should be gamma corrected.
    // TODO: Use more accurate gamma correction.
    var outColor = vec4<f32>(1.0);
    switch (render_settings.debug_mode.x) {
        case 1: {
            let color = normalize(in.position.xyz) * 0.5 + 0.5;
            outColor = vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 2: {
            let color = normalize(in.normal.xyz) * 0.5 + 0.5;
            outColor = vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 3: {
            let color = normalize(in.tangent.xyz) * 0.5 + 0.5;
            outColor = vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
        }
        case 4: {
            outColor = colorSet1;
        }
        case 5: {
            outColor = colorSet2;
        }
        case 6: {
            outColor = colorSet3;
        }
        case 7: {
            outColor = colorSet4;
        }
        case 8: {
            outColor = colorSet5;
        }
        case 9: {
            outColor = colorSet6;
        }
        case 10: {
            outColor = colorSet7;
        }
        case 11: {
            outColor = textureSample(texture0, sampler0, map1);
        }
        case 12: {
            outColor = textureSample(texture1, sampler1, uvSet);
        }
        case 13: {
            outColor = textureSample(texture2, sampler2, reflectionVector);
        }
        case 14: {
            outColor = textureSample(texture3, sampler3, bake1);
        }
        case 15: {
            outColor = textureSample(texture4, sampler4, map1);
        }
        case 16: {
            outColor = textureSample(texture5, sampler5, map1);
        }
        case 17: {
            outColor = textureSample(texture6, sampler6, map1);
        }
        case 18: {
            outColor = textureSample(texture7, sampler7, reflectionVector);
        }
        case 19: {
            outColor = textureSample(texture8, sampler8, reflectionVector);
        }
        case 20: {
            outColor = textureSample(texture9, sampler9, bake1);
        }
        case 21: {
            outColor = textureSample(texture10, sampler10, map1);
        }
        case 22: {
            outColor = textureSample(texture11, sampler11, uvSet);
        }
        case 23: {
            outColor = textureSample(texture12, sampler12, map1);
        }
        case 24: {
            outColor = textureSample(texture13, sampler13, map1);
        }
        case 25: {
            outColor = textureSample(texture14, sampler14, uvSet);
        }
        // case 26: {
        //     outColor = textureSample(texture16, sampler16, map1);
        // }
        case 27: {
            outColor = vec4<f32>(pow(map1, vec2<f32>(2.2)), 1.0, 1.0);
        }
        case 28: {
            outColor = vec4<f32>(pow(bake1, vec2<f32>(2.2)), 1.0, 1.0);
        }
        case 29: {
            outColor = vec4<f32>(pow(uvSet, vec2<f32>(2.2)), 1.0, 1.0);
        }
        case 30: {
            outColor = vec4<f32>(pow(uvSet1, vec2<f32>(2.2)), 1.0, 1.0);
        }
        case 31: {
            outColor = vec4<f32>(pow(uvSet2, vec2<f32>(2.2)), 1.0, 1.0);
        }
        case 32: {
            // Basic Shading.
            let basic = 0.218 * max(dot(fragmentNormal, viewVector), 0.0);
            outColor = vec4<f32>(vec3<f32>(basic), 1.0);
        }
        case 33: {
            // Normals
            outColor = vec4<f32>(pow(fragmentNormal.xyz * 0.5 + 0.5, vec3<f32>(2.2)), 1.0);
        }
        case 34: {
            // Bitangents
            outColor = vec4<f32>(pow(bitangent.xyz * 0.5 + 0.5, vec3<f32>(2.2)), 1.0);
        }
        case 35: {
            // Albedo
            outColor = vec4<f32>(albedoColorFinal.rgb, 1.0);
        }
        default: { 
            outColor = vec4<f32>(1.0);
        }
    }

    // Use grayscale for single channels.
    let rgba = render_settings.render_rgba;
    if (rgba.r == 1.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.rrr, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 1.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.ggg, 1.0);
    }

    if (rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 1.0) {
        return vec4<f32>(outColor.bbb, 1.0);
    }

    if (rgba.a == 1.0 && rgba.r == 0.0 && rgba.g == 0.0 && rgba.b == 0.0) {
        return vec4<f32>(outColor.aaa, 1.0);
    }

    return vec4<f32>(outColor.rgb * rgba.rgb, 1.0);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput, [[builtin(front_facing)]] is_front: bool) -> [[location(0)]] vec4<f32> {
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

    // TODO: Mario's eyes don't render properly for the metal/gold materials.
    var nor = vec4<f32>(0.5, 0.5, 1.0, 1.0);
    if (uniforms.has_texture[4].x == 1u) {
        nor = textureSample(texture4, sampler4, map1);
    }

    // Check if the factor is non zero to prevent artifacts.
    var transitionFactor = 0.0;
    if ((render_settings.transition_factor.x > 0.0) && (nor.b >= (1.0 - render_settings.transition_factor.x))) {
        transitionFactor = 1.0;
    }

    // TODO: Finish researching these values from in game.
    // TODO: Apply the metal/metamon materials.
    var transitionAlbedo = vec3<f32>(0.0);
    var transitionPrm = vec4<f32>(0.0);
    var transitionCustomVector11 = vec4<f32>(0.0);
    var transitionCustomVector30 = vec4<f32>(0.0);

    switch (render_settings.transition_material.x) {
        case 0: {      
            // Inkling's Ink.
            // TODO: Include other colors from /fighter/common/param/effect.prc?
            transitionAlbedo = vec3<f32>(0.758027, 0.115859, 0.04);
            // TODO: Ink PRM?
            transitionPrm = vec4<f32>(0.0, 0.2, 1.0, 0.16);
            transitionCustomVector11 = vec4<f32>(0.0);
            transitionCustomVector30 = vec4<f32>(0.0);
        }
        case 1: {      
            // Metal Box.
            transitionAlbedo = vec3<f32>(0.257, 0.257, 0.257);
            transitionPrm = vec4<f32>(1.0, 0.3, 1.0, 0.0);
            transitionCustomVector11 = vec4<f32>(0.0);
            transitionCustomVector30 = vec4<f32>(0.0);
        }
        case 2: {      
            // Gold (Xerneas Pokemon).
            // (0.257, 0.257, 0.257) + (0.125, 0.047, -0.234) in the shader.
            transitionAlbedo = vec3<f32>(0.382, 0.304, 0.023);
            transitionPrm = vec4<f32>(1.0, 0.3, 1.0, 0.0);
            transitionCustomVector11 = vec4<f32>(0.0);
            transitionCustomVector30 = vec4<f32>(0.0);
        }
        case 3: {      
            // Ditto Pokemon.
            transitionAlbedo = vec3<f32>(0.1694, 0.0924, 0.2002);
            transitionPrm = vec4<f32>(1.0, 0.75, 1.0, 0.032); // TODO: Roughness?
            transitionCustomVector11 = vec4<f32>(0.0); // TODO: What is this?
            transitionCustomVector30 = vec4<f32>(0.5, 4.0, 0.0, 0.0);
        }
        default: { 
            
        }
    }

    let customVector11Final = mix(uniforms.custom_vector[11], transitionCustomVector11, render_settings.transition_factor.x);
    let customVector30Final = mix(uniforms.custom_vector[30], transitionCustomVector30, render_settings.transition_factor.x);

    // TODO: Some materials disable specular entirely?
    // Is there a reliably way to check for this?
    var prm = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    if (uniforms.has_texture[6].x == 1u) {
        prm = textureSample(texture6, sampler6, map1);
    }

    var metalness = mix(prm.r, transitionPrm.r, transitionFactor);
    let roughness = mix(prm.g, transitionPrm.g, transitionFactor);
    let ao = prm.b;
    let spec = mix(prm.a, transitionPrm.a, transitionFactor);

    let sssBlend = prm.r * customVector30Final.x;

    // Skin shaders use metalness for masking the fake SSS effect.
    if (customVector30Final.x > 0.0) {
        metalness = 0.0;
    }

    let viewVector = normalize(camera.camera_pos.xyz - in.position.xyz);

    let normal = normalize(in.normal.xyz);
    let tangent = normalize(in.tangent.xyz);
    let bitangent = normalize(cross(normal, tangent)) * in.tangent.w * -1.0;

    var fragmentNormal = normal;
    if (uniforms.has_texture[4].x == 1u) {
        fragmentNormal = GetBumpMapNormal(normal, tangent, bitangent, nor);
    }

    // TODO: Investigate lighting for double sided materials with culling disabled.
    if (!is_front) {
        fragmentNormal = fragmentNormal * -1.0;
    }

    // TODO: Is it just the metal material that uses the fragment normal?
    var reflectionVector = reflect(viewVector, fragmentNormal);
    reflectionVector.y = reflectionVector.y * -1.0;

    let chrLightDir = stage_uniforms.chr_light_dir.xyz;

    let halfAngle = normalize(chrLightDir + viewVector);
    let nDotV = max(dot(fragmentNormal, viewVector), 0.0);
    let nDotH = clamp(dot(fragmentNormal, halfAngle), 0.0, 1.0);
    let nDotL = dot(fragmentNormal, normalize(chrLightDir));

    let albedoColor = GetAlbedoColor(map1, uvSet, uvSet1, reflectionVector, colorSet5);
    var albedoColorFinal = GetAlbedoColorFinal(albedoColor);

    albedoColorFinal = mix(albedoColorFinal, transitionAlbedo, transitionFactor);

    let emissionColor = GetEmissionColor(map1, uvSet);

    var shadow = 1.0;
    if (render_settings.render_shadows.x == 1u) {
        shadow = GetShadow(in.light_position);
    }

    var outAlpha = max(albedoColor.a * emissionColor.a, uniforms.custom_vector[0].x);
    if (uniforms.is_discard.x == 1u && outAlpha < 0.5) {
        discard;
    }

    let specularF0 = GetF0FromSpecular(prm.a);

    let specularLod = RoughnessToLod(roughness);
    let specularIbl = textureSampleLevel(texture7, sampler7, reflectionVector, specularLod).rgb;

    // TODO: Vertex shader
    let shAmbientR = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.14186, 0.04903, -0.082, 1.11054));
    let shAmbientG = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.14717, 0.03699, -0.08283, 1.11036));
    let shAmbientB = dot(vec4<f32>(normalize(normal), 1.0), vec4<f32>(0.1419, 0.04334, -0.08283, 1.11018));
    let shColor = vec3<f32>(shAmbientR, shAmbientG, shAmbientB);

    let diffusePass = DiffuseTerm(bake1, albedoColorFinal.rgb, nDotL, shColor, vec3<f32>(ao), sssBlend, shadow, customVector11Final, customVector30Final, colorSet2);

    let specularPass = SpecularTerm(in.tangent, nDotH, max(nDotL, 0.0), nDotV, halfAngle, fragmentNormal, roughness, specularIbl, metalness, prm.a, shadow);

    let kSpecular = GetSpecularWeight(specularF0, albedoColorFinal.rgb, metalness, nDotV, roughness);

    var kDiffuse = max((vec3<f32>(1.0) - kSpecular) * (1.0 - metalness), vec3<f32>(0.0));
    kDiffuse = max(vec3<f32>(1.0 - metalness), vec3<f32>(0.0));

    var outColor = vec3<f32>(0.0, 0.0, 0.0);
    if (render_settings.render_diffuse.x == 1u) {
        outColor = outColor + (diffusePass * kDiffuse) / 3.14159;
    }

    if (render_settings.render_specular.x == 1u) {
        outColor = outColor + specularPass * kSpecular * ao;
    }

    if (render_settings.render_emission.x == 1u) {
        // TODO: Emission is weakened somehow?
        outColor = outColor + EmissionTerm(emissionColor) * 0.5;
    }

    // TODO: What affects rim lighting intensity?
    if (render_settings.render_rim_lighting.x == 1u) {
        let rimOcclusion = shadow;
        outColor = GetRimBlend(outColor, albedoColorFinal, nDotV, max(nDotL, 0.0), rimOcclusion, shColor);
    }

    if (uniforms.has_vector[8].x == 1u) {
        // TODO: Does this affect alpha?
        outColor = outColor * uniforms.custom_vector[8].rgb;
    }

    if (uniforms.has_color_set1234.x == 1u) {
        outColor = outColor * colorSet1.rgb; 
        outAlpha = outAlpha * colorSet1.a; 
    }

    if (uniforms.has_color_set1234.z == 1u) {
        outColor = outColor * colorSet3.rgb;
        outAlpha = outAlpha * colorSet3.a; 
    }

    if (uniforms.has_float[19].x == 1u) {
        outAlpha = GetAngleFade(nDotV, uniforms.custom_float[19].x, specularF0);
    }

    // Premultiplied alpha. 
    // TODO: This is only for some materials.
    outColor = outColor * outAlpha;

    // Alpha override.
    if (uniforms.has_boolean[2].x == 1u && uniforms.custom_boolean[2].x == 1u) {
        outAlpha = 0.0;
    }

    return vec4<f32>(outColor, outAlpha);
}