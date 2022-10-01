use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ShaderProgram {
    pub discard: bool,
    pub vertex_attributes: Vec<String>,
    pub material_parameters: Vec<String>,
}

impl ShaderProgram {
    /// Returns `true` if `attributes` has all the vertex attributes required by this shader program.
    // TODO: Take an iterator instead?
    pub fn has_required_attributes(&self, attributes: &[String]) -> bool {
        self.vertex_attributes
            .iter()
            .filter(|a| a.as_str() != "ink_color_set")
            .all(|a| attributes.contains(a))
    }

    /// Returns the vertex attribute names required by this shader program not present in `attributes`.
    // TODO: Take an iterator instead?
    pub fn missing_required_attributes(&self, attributes: &[String]) -> Vec<String> {
        self.vertex_attributes
            .iter()
            .filter(|a| a.as_str() != "ink_color_set" && !attributes.contains(a))
            .map(String::to_string)
            .collect()
    }

    /// Returns the color channels accessed by the shaders as `[x, y, z, w]`.
    pub fn accessed_channels(&self, param_name: &str) -> [bool; 4] {
        let mut channels = [false; 4];
        if let Some(database_param) = self
            .material_parameters
            .iter()
            .find(|p| p.starts_with(&param_name))
        {
            let (_, components) = split_param(database_param);
            for (i, c) in "xyzw".chars().enumerate() {
                channels[i] = components.contains(c);
            }
        }
        channels
    }
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
    /// Creates a database with the specified shader programs.
    pub fn from_iter<I>(programs: I) -> Self
    where
        I: Iterator<Item = (String, ShaderProgram)>,
    {
        Self(HashMap::from_iter(programs))
    }

    /// Creates the shader database used for Smash Ultimate.
    pub fn new() -> Self {
        let mut programs = HashMap::with_capacity(4008);

        let v: Value = serde_json::from_str(SHADER_JSON).unwrap();
        for program in v["shaders"].as_array().unwrap() {
            programs.insert(
                program["name"].as_str().unwrap().to_string(),
                ShaderProgram {
                    discard: program["discard"].as_bool().unwrap(),
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
                },
            );
        }

        ShaderDatabase(programs)
    }

    /// Get the shader with the specified `shader_label` while ignoring tags like `"_opaque"`.
    pub fn get(&self, shader_label: &str) -> Option<&ShaderProgram> {
        self.0.get(shader_label.get(..24).unwrap_or(""))
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
    fn has_required_attributes_empty() {
        assert!(ShaderProgram {
            discard: false,
            vertex_attributes: Vec::new(),
            material_parameters: Vec::new(),
        }
        .has_required_attributes(&[]));
    }

    #[test]
    fn has_required_attributes_extras() {
        assert!(ShaderProgram {
            discard: false,
            vertex_attributes: Vec::new(),
            material_parameters: Vec::new(),
        }
        .has_required_attributes(&["abc".to_string()]));
    }

    #[test]
    fn has_required_attributes_missing() {
        assert!(!ShaderProgram {
            discard: false,
            vertex_attributes: vec!["a".to_string(), "b".to_string()],
            material_parameters: Vec::new(),
        }
        .has_required_attributes(&["a".to_string()]));
    }

    #[test]
    fn has_required_attributes() {
        assert!(ShaderProgram {
            discard: false,
            vertex_attributes: vec!["a".to_string(), "b".to_string()],
            material_parameters: Vec::new(),
        }
        .has_required_attributes(&["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn has_required_attributes_ink_color_set() {
        // Check that "ink_color_set" is ignored since it isn't part of the mesh
        // TODO: Investigate how this attribute is generated.
        assert!(ShaderProgram {
            discard: false,
            vertex_attributes: vec!["ink_color_set".to_string(), "map1".to_string()],
            material_parameters: Vec::new(),
        }
        .has_required_attributes(&["map1".to_string()]));
    }

    #[test]
    fn missing_required_attributes_empty() {
        // Check that "ink_color_set" is ignored since it isn't part of the mesh
        // TODO: Investigate how this attribute is generated.
        assert!(ShaderProgram {
            discard: false,
            vertex_attributes: Vec::new(),
            material_parameters: Vec::new(),
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
                discard: false,
                vertex_attributes: vec!["ink_color_set".to_string(), "map1".to_string()],
                material_parameters: Vec::new(),
            }
            .missing_required_attributes(&[])
        );
    }
}
