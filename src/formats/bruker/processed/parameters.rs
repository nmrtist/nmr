use crate::read_error::ReadError;
use crate::read_error::ReadResource;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct Parameters {
    values: BTreeMap<String, String>,
    retained: Vec<(String, String)>,
}

impl Parameters {
    pub(super) fn parse(text: &str, source: &Path) -> Result<Self, ReadError> {
        let mut values = BTreeMap::new();
        let mut retained = Vec::new();
        let mut current: Option<(String, String)> = None;
        for line in text.lines() {
            if line.starts_with("$$") {
                continue;
            }
            if line.starts_with("##") {
                if let Some((name, value)) = current.take() {
                    insert_unique(&mut values, &mut retained, name, value, source)?;
                }
            }
            if let Some(record) = line.strip_prefix("##$") {
                let (name, value) = record.split_once('=').ok_or_else(|| {
                    ReadError::corrupt(source.into(), "malformed Bruker parameter record")
                })?;
                current = Some((name.trim().to_ascii_uppercase(), value.trim().to_owned()));
            } else if !line.starts_with("##") {
                if let Some((_, value)) = &mut current {
                    if !value.is_empty() {
                        value.push(' ');
                    }
                    value.push_str(line.trim());
                }
            }
        }
        if let Some((name, value)) = current {
            insert_unique(&mut values, &mut retained, name, value, source)?;
        }
        Ok(Self { values, retained })
    }

    pub(super) fn usize(&self, name: &str, source: &Path) -> Result<usize, ReadError> {
        self.value(name, source)?
            .parse::<usize>()
            .map_err(|_| invalid_parameter(source, name))
    }

    pub(super) fn i32(&self, name: &str, source: &Path) -> Result<i32, ReadError> {
        self.value(name, source)?
            .parse::<i32>()
            .map_err(|_| invalid_parameter(source, name))
    }

    pub(super) fn optional_i32(&self, name: &str, source: &Path) -> Result<Option<i32>, ReadError> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| invalid_parameter(source, name))
            })
            .transpose()
    }

    pub(super) fn optional_f64(&self, name: &str, source: &Path) -> Result<Option<f64>, ReadError> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| invalid_parameter(source, name))
            })
            .transpose()
    }

    pub(super) fn text(&self, name: &str) -> Option<String> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .trim()
                    .trim_matches(|character| character == '<' || character == '>')
                    .trim()
                    .to_owned()
            })
            .filter(|value| !value.is_empty())
    }

    pub(super) fn value<'a>(&'a self, name: &str, source: &Path) -> Result<&'a str, ReadError> {
        self.values.get(name).map(String::as_str).ok_or_else(|| {
            ReadError::incomplete(
                source.into(),
                format!("required parameter {name} is missing"),
            )
        })
    }
}

pub(super) fn insert_unique(
    values: &mut BTreeMap<String, String>,
    retained: &mut Vec<(String, String)>,
    name: String,
    value: String,
    source: &Path,
) -> Result<(), ReadError> {
    if values.insert(name.clone(), value.clone()).is_some() {
        return Err(ReadError::corrupt(
            source.into(),
            format!("duplicate parameter {name}"),
        ));
    }
    retained.push((name, value));
    Ok(())
}

pub(super) fn invalid_parameter(source: &Path, name: &str) -> ReadError {
    ReadError::corrupt(
        source.into(),
        format!("parameter {name} has an invalid value"),
    )
}

pub(super) fn read_metadata(
    control: &mut crate::ExecutionContext<'_>,
    path: &Path,
    limit: usize,
) -> Result<String, ReadError> {
    let length = fs::metadata(path)
        .map_err(|error| ReadError::io(path, error))?
        .len();
    let length = usize::try_from(length).map_err(|_| ReadError::SizeOverflow)?;
    if length > limit {
        return Err(ReadError::limit(ReadResource::MetadataBytes, limit, length));
    }
    let bytes = crate::io::read_sized_file_controlled(control, path, length)?;
    String::from_utf8(bytes).map_err(|_| ReadError::corrupt(path.into(), "metadata is not UTF-8"))
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl Parameters {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&BTreeMap<String, String>, &Vec<(String, String)>) {
        (&self.values, &self.retained)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (BTreeMap<String, String>, Vec<(String, String)>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (values, retained) = parts;
        let value = Self { values, retained };

        Ok(value)
    }
}

impl Parameters {
    pub(super) fn into_retained(self) -> Vec<(String, String)> {
        self.retained
    }
}
