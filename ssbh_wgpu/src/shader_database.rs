use serde_json::Value;
use ssbh_data::matl_data::ParamId;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ShaderProgram {
    pub discard: bool,
    pub vertex_attributes: Vec<String>,
    pub material_parameters: Vec<ParamId>,
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
}

static SHADER_JSON: &str = include_str!("shaders.json");

pub type ShaderDatabase = HashMap<String, ShaderProgram>;

pub fn create_database() -> ShaderDatabase {
    let mut database = HashMap::with_capacity(4008);

    let v: Value = serde_json::from_str(SHADER_JSON).unwrap();
    for program in v["shaders"].as_array().unwrap() {
        database.insert(
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
                    .map(|v| ParamId::from_repr(v.as_u64().unwrap() as usize).unwrap())
                    .collect(),
            },
        );
    }

    database
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_discard() {
        let database = create_database();
        assert!(!database.get("SFX_PBS_011000000800826b").unwrap().discard);
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
