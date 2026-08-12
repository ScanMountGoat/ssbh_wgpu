use std::collections::BTreeMap;

use smush_shader::ShaderProgram;

static DATABASE_BINARY: &[u8] = include_bytes!("resources/shaders.bin");

pub struct ShaderDatabase(pub smush_shader::ShaderDatabase);

impl ShaderDatabase {
    /// Creates the shader database used for Smash Ultimate.
    pub fn new() -> Self {
        // Unwrap is safe since we load a static file.
        let database = smush_shader::ShaderDatabase::from_bytes(DATABASE_BINARY).unwrap();
        ShaderDatabase(database)
    }

    /// Get the shader with the specified `shader_label` while ignoring tags like `"_opaque"`.
    pub fn get(&self, shader_label: &str) -> Option<&ShaderProgram> {
        self.0.get_shader(shader_label.get(..24).unwrap_or(""))
    }
}

impl Default for ShaderDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<(String, ShaderProgram)> for ShaderDatabase {
    fn from_iter<T: IntoIterator<Item = (String, ShaderProgram)>>(iter: T) -> Self {
        Self(smush_shader::ShaderDatabase::from_programs(
            BTreeMap::from_iter(iter.into_iter().map(|(k, v)| (k.into(), v))),
        ))
    }
}
