use super::builder::ConfigBuilder;
use super::format::{deserialize, Format};
use super::TransformOuter;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct Pipelines(pub(crate) IndexMap<String, Pipeline>);

impl From<IndexMap<String, Pipeline>> for Pipelines {
    fn from(value: IndexMap<String, Pipeline>) -> Self {
        Self(value)
    }
}

// Validation related
impl Pipelines {
    pub(crate) fn transform_keys<'a>(&'a self) -> impl Iterator<Item = (&'a String, &'a String)> {
        self.0
            .iter()
            .map(|(pipeline_id, pipeline)| {
                pipeline
                    .transforms
                    .keys()
                    .map(move |name| (pipeline_id, name))
            })
            .flatten()
    }

    pub(crate) fn outputs<'a>(&'a self) -> impl Iterator<Item = &'a String> {
        self.0
            .iter()
            .map(|(_id, pipeline)| pipeline.transforms.values())
            .flatten()
            .map(|transform| transform.outputs.iter())
            .flatten()
    }

    pub(crate) fn check_shape(&self, config: &ConfigBuilder, errors: &mut Vec<String>) {
        self.check_inputs(config, errors);
        self.check_outputs(config, errors);
    }

    fn check_outputs(&self, config: &ConfigBuilder, errors: &mut Vec<String>) {
        self.0.iter().for_each(|(pipeline_id, pipeline)| {
            pipeline.check_outputs(pipeline_id, config, errors)
        });
    }

    fn check_inputs(&self, config: &ConfigBuilder, errors: &mut Vec<String>) {
        self.0
            .iter()
            .for_each(|(pipeline_id, pipeline)| pipeline.check_inputs(pipeline_id, config, errors));
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct PipelineTransform {
    #[serde(flatten)]
    pub inner: TransformOuter,
    #[serde(flatten)]
    pub outputs: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    #[serde(default)]
    pub transforms: IndexMap<String, PipelineTransform>,
}

// Loading related
impl Pipeline {
    pub fn load_from_folder(folder: &PathBuf) -> Result<IndexMap<String, Self>, Vec<String>> {
        let entries = fs::read_dir(folder)
            .map_err(|err| {
                vec![format!(
                    "Could not list folder content: {:?}, {}",
                    folder, err
                )]
            })?
            .map(|entry| entry.unwrap().path())
            .filter(|entry| entry.is_file())
            .map(|entry| Self::load_from_file(&entry));
        let mut index = IndexMap::new();
        let mut errors = Vec::new();
        for entry in entries {
            match entry {
                Ok((id, pipeline)) => {
                    index.insert(id, pipeline);
                }
                Err(err) => {
                    errors.push(err);
                }
            };
        }
        if errors.is_empty() {
            Ok(index)
        } else {
            Err(errors)
        }
    }

    pub fn load_from_file(file: &PathBuf) -> Result<(String, Self), String> {
        let format =
            Format::from_path(file).map_err(|err| format!("Could not read format: {:?}", err))?;
        let filename = file
            .file_stem()
            .and_then(|name| name.to_str().map(ToString::to_string))
            .ok_or_else(|| format!("Could not read filename: {:?}", file))?;
        let content = fs::read_to_string(file)
            .map_err(|err| format!("Could not read content: {:?}, {}", file, err))?;
        deserialize(&content, Some(format))
            .map(|value| (filename, value))
            .map_err(|err| format!("Could not parse content: {:?}, {:?}", file, err))
    }
}

// Validation related
impl Pipeline {
    fn check_inputs(&self, pipeline_id: &str, config: &ConfigBuilder, errors: &mut Vec<String>) {
        self.transforms
            .iter()
            .map(|(name, transform)| {
                transform
                    .inner
                    .inputs
                    .iter()
                    .filter(|input| {
                        !config.has_input(&input) && !self.transforms.contains_key(input.as_str())
                    })
                    .map(move |input| (name, input))
            })
            .flatten()
            .for_each(|(name, input)| {
                errors.push(format!(
                    "Input {:?} for transform {:?} in pipeline {:?} doesn't exist.",
                    input, name, pipeline_id
                ));
            });
    }

    fn check_outputs(&self, pipeline_id: &str, config: &ConfigBuilder, errors: &mut Vec<String>) {
        self.transforms
            .iter()
            .map(|(name, transform)| {
                transform
                    .outputs
                    .iter()
                    .filter(|input| !config.has_output(&input))
                    .map(move |input| (name, input))
            })
            .flatten()
            .for_each(|(name, input)| {
                errors.push(format!(
                    "Output {:?} for transform {:?} in pipeline {:?} doesn't exist.",
                    input, name, pipeline_id
                ));
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        let src = r#"
        [transforms.first]
        inputs = ["input"]
        outputs = ["output"]
        type = "remap"
        source = ""
        "#;
        let result: Pipeline = deserialize(src, Some(Format::Toml)).unwrap();
        assert_eq!(result.transforms.len(), 1);
    }
}
