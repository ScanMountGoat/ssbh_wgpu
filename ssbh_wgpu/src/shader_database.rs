use serde_json::Value;
use ssbh_data::matl_data::ParamId;
use std::collections::HashMap;

#[derive(Debug)]
pub struct ShaderProgram {
    pub discard: bool,
    pub vertex_attributes: Vec<String>,
    pub material_parameters: Vec<ParamId>,
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
