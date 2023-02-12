use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ShaderProgram {
    /// `true` if the code contains "discard;" and likely has alpha testing.
    pub discard: bool,
    /// `true` if the fragment RGB outputs are multiplied by the alpha output value.
    pub premultiplied: bool,
    /// `true` if the fragment shader has a shadow map texture and will render shadows.
    /// This does not affect casting shadows on other objects.
    pub receives_shadow: bool,
    /// The collection of required mesh vertex attributes and their accessed channels.
    pub vertex_attributes: Vec<String>,
    /// The collection of required material parameters and their accessed channels.
    pub material_parameters: Vec<String>,
    /// A heuristic for shader complexity in the range `0.0` to `1.0`.
    pub complexity: f64,
}

impl ShaderProgram {
    /// Returns `true` if `attributes` has all the vertex attributes required by this shader program.
    // TODO: Take an iterator instead?
    pub fn has_required_attributes(&self, attributes: &[String]) -> bool {
        self.vertex_attributes
            .iter()
            .map(|a| attribute_name_no_channels(a))
            .filter(|a| *a != "ink_color_set")
            .all(|required| attributes.iter().any(|a| a == required))
    }

    /// Returns the vertex attribute names required by this shader program not present in `attributes`.
    // TODO: Take an iterator instead?
    pub fn missing_required_attributes(&self, attributes: &[String]) -> Vec<String> {
        self.vertex_attributes
            .iter()
            .map(|a| attribute_name_no_channels(a))
            .filter(|required| {
                *required != "ink_color_set" && !attributes.iter().any(|a| a == required)
            })
            .map(|a| a.to_string())
            .collect()
    }

    /// Returns the color channels accessed by the shaders as `[x, y, z, w]`.
    pub fn accessed_channels(&self, param_name: &str) -> [bool; 4] {
        let mut channels = [false; 4];
        if let Some(database_param) = self
            .material_parameters
            .iter()
            .find(|p| p.starts_with(param_name))
        {
            let (_, components) = split_param(database_param);
            for (i, c) in "xyzw".chars().enumerate() {
                channels[i] = components.contains(c);
            }
        }
        channels
    }

    /// Returns `true` if this program requires `attribute`.
    pub fn has_attribute(&self, attribute: &str) -> bool {
        self.vertex_attributes
            .iter()
            .map(|a| attribute_name_no_channels(a))
            .any(|a| a == attribute)
    }
}

fn attribute_name_no_channels(attribute: &str) -> &str {
    // "map1.xy" -> "map1"
    // "map1" -> "map1"
    attribute.split_once('.').map(|a| a.0).unwrap_or(attribute)
}

/// Splits `param` into its parameter name and accessed components.
///
/// # Examples
/**
```rust
use ssbh_wgpu::split_param;

assert_eq!(("CustomBoolean3", ""), split_param("CustomBoolean3"));
assert_eq!(("CustomVector0", "x"), split_param("CustomVector0.x"));
assert_eq!(("CustomVector12", ""), split_param("CustomVector12."));
*/
pub fn split_param(param: &str) -> (&str, &str) {
    param
        .find('.')
        .map(|i| {
            (
                param.get(..i).unwrap_or(""),
                param.get(i + 1..).unwrap_or(""),
            )
        })
        .unwrap_or((param, ""))
}

static SHADER_JSON: &str = include_str!("shaders.json");

pub struct ShaderDatabase(HashMap<String, ShaderProgram>);

impl ShaderDatabase {
    /// Creates the shader database used for Smash Ultimate.
    pub fn new() -> Self {
        // Unwrap is safe since we load a static JSON file.
        let v: Value = serde_json::from_str(SHADER_JSON).unwrap();
        let programs = v["shaders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|program| {
                (
                    program["name"].as_str().unwrap().to_string(),
                    ShaderProgram {
                        discard: program["discard"].as_bool().unwrap(),
                        premultiplied: program["premultiplied"].as_bool().unwrap(),
                        receives_shadow: program["receives_shadow"].as_bool().unwrap(),
                        vertex_attributes: program["attrs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect(),
                        material_parameters: program["params"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect(),
                        complexity: program["complexity"].as_f64().unwrap(),
                    },
                )
            })
            .collect();

        ShaderDatabase(programs)
    }

    /// Get the shader with the specified `shader_label` while ignoring tags like `"_opaque"`.
    pub fn get(&self, shader_label: &str) -> Option<&ShaderProgram> {
        self.0.get(shader_label.get(..24).unwrap_or(""))
    }
}

impl Default for ShaderDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<(String, ShaderProgram)> for ShaderDatabase {
    fn from_iter<T: IntoIterator<Item = (String, ShaderProgram)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_discard() {
        let database = ShaderDatabase::new();
        assert!(
            !database
                .get("SFX_PBS_011000000800826b_IGNORE_TAGS")
                .unwrap()
                .discard
        );
        assert!(database.get("SFX_PBS_010000000804826b").unwrap().discard);
        assert!(database.get("SFX_PBS_010000000804830d").unwrap().discard);
    }

    #[test]
    fn program_premultiplied() {
        let database = ShaderDatabase::new();
        assert!(
            database
                .get("SFX_PBS_3801000002018240_IGNORE_TAGS")
                .unwrap()
                .premultiplied
        );
        assert!(
            database
                .get("SFX_PBS_3801000002018240")
                .unwrap()
                .premultiplied
        );
        assert!(
            !database
                .get("SFX_PBS_0100000008008269")
                .unwrap()
                .premultiplied
        );
    }

    #[test]
    fn has_required_attributes_empty() {
        assert!(ShaderProgram {
            vertex_attributes: Vec::new(),
            ..Default::default()
        }
        .has_required_attributes(&[]));
    }

    #[test]
    fn has_required_attributes_extras() {
        assert!(ShaderProgram {
            vertex_attributes: Vec::new(),
            ..Default::default()
        }
        .has_required_attributes(&["abc".to_string()]));
    }

    #[test]
    fn has_required_attributes_missing() {
        assert!(!ShaderProgram {
            vertex_attributes: vec!["a".to_string(), "b".to_string()],
            ..Default::default()
        }
        .has_required_attributes(&["a".to_string()]));
    }

    #[test]
    fn has_required_attributes() {
        // Make sure the channel extensions are ignored.
        assert!(ShaderProgram {
            vertex_attributes: vec!["a.xz".to_string(), "b.w".to_string()],
            ..Default::default()
        }
        .has_required_attributes(&["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn has_required_attributes_ink_color_set() {
        // Check that "ink_color_set" is ignored since it isn't part of the mesh
        // TODO: Investigate how this attribute is generated.
        assert!(ShaderProgram {
            vertex_attributes: vec!["ink_color_set".to_string(), "map1".to_string()],
            ..Default::default()
        }
        .has_required_attributes(&["map1".to_string()]));
    }

    #[test]
    fn missing_required_attributes_empty() {
        // Check that "ink_color_set" is ignored since it isn't part of the mesh
        // TODO: Investigate how this attribute is generated.
        assert!(ShaderProgram {
            vertex_attributes: Vec::new(),
            ..Default::default()
        }
        .missing_required_attributes(&[])
        .is_empty());
    }

    #[test]
    fn missing_required_attributes_ink_color_set() {
        // Check that "ink_color_set" is ignored since it isn't part of the mesh
        // TODO: Investigate how this attribute is generated.
        assert_eq!(
            vec!["map1".to_string()],
            ShaderProgram {
                vertex_attributes: vec!["ink_color_set".to_string(), "map1.xy".to_string()],
                ..Default::default()
            }
            .missing_required_attributes(&[])
        );
    }
}
