use crate::processed;
use crate::raw::RawFormat;
use crate::read_error::{ReadCandidate, ReadError};
use crate::{DatasetIdentity, Format, ReadPreference};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct ResolvedCandidate {
    pub(crate) format: Format,
    pub(crate) root: PathBuf,
    pub(crate) primary: PathBuf,
    pub(crate) companions: Vec<(PathBuf, &'static str)>,
    pub(crate) optional_companions: Vec<(PathBuf, &'static str)>,
}

impl ResolvedCandidate {
    pub(crate) fn validate_required(&self) -> Result<(), ReadError> {
        for (path, role) in self.required_sources() {
            if !path.is_file() {
                return Err(ReadError::incomplete(
                    self.root.clone().into(),
                    format!("required {role} is missing at {}", path.display()),
                )
                .with_format(self.format));
            }
        }
        Ok(())
    }

    fn is_structurally_complete(&self) -> bool {
        self.required_sources().all(|(path, _)| path.is_file())
    }

    fn required_sources(&self) -> impl Iterator<Item = (&Path, &str)> {
        std::iter::once((self.primary.as_path(), "primary data")).chain(
            self.companions
                .iter()
                .map(|(path, role)| (path.as_path(), *role)),
        )
    }

    pub(crate) fn identity(&self, acquisition: Option<String>) -> DatasetIdentity {
        match self.format.family() {
            crate::FormatFamily::BrukerTopSpin => {
                let experiment = if self.format.kind() == crate::DatasetKind::Processed {
                    self.root
                        .parent()
                        .filter(|path| lowercase_name(path) == "pdata")
                        .and_then(Path::parent)
                } else {
                    Some(self.root.as_path())
                };
                let subject = experiment
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .map(|value| value.to_string_lossy().into_owned());
                let source_label = experiment
                    .and_then(Path::file_name)
                    .map(|value| value.to_string_lossy().into_owned());
                DatasetIdentity::new(subject, acquisition, source_label)
            }
            crate::FormatFamily::Varian => {
                let label = self
                    .root
                    .file_name()
                    .map(|value| value.to_string_lossy().trim_end_matches(".fid").to_owned());
                DatasetIdentity::new(label, acquisition, None)
            }
            crate::FormatFamily::JeolDelta => DatasetIdentity::new(None, acquisition, None),
            crate::FormatFamily::JcampDx => DatasetIdentity::default(),
        }
    }

    pub(crate) fn companion(&self, role: &str) -> Option<&Path> {
        self.companions
            .iter()
            .chain(&self.optional_companions)
            .find_map(|(path, candidate_role)| (*candidate_role == role).then_some(path.as_path()))
    }

    pub(crate) fn open_raw(
        &self,
        limits: &crate::ReadLimits,
        varian: Option<&crate::formats::varian::ReadAssertions>,
        experimental_vendor_semantics: bool,
        sampling: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
        cancellation: &crate::CancellationToken,
    ) -> Result<crate::raw::Reader, ReadError> {
        match self.format {
            Format::Raw(RawFormat::BrukerRaw) => {
                if sampling.is_some() {
                    crate::formats::bruker::require_experimental_nus(
                        experimental_vendor_semantics,
                        self.primary.clone().into(),
                    )?;
                }
                if let Some(path) = self.companion("nuslist").filter(|path| path.is_file()) {
                    crate::formats::bruker::require_experimental_nus(
                        experimental_vendor_semantics,
                        path.into(),
                    )
                    .map_err(|error| error.with_format(self.format))?;
                }
                let mut parameters = Vec::new();
                for role in ["acqus", "acqu2s", "acqu3s"] {
                    if let Some(path) = self.companion(role).filter(|path| path.is_file()) {
                        parameters.push(path.to_path_buf());
                    }
                }
                crate::formats::bruker::open_resolved_acquisition_declared(
                    &self.primary,
                    &parameters,
                    self.companion("nuslist"),
                    limits,
                    sampling,
                    cancellation,
                )
            }
            Format::Raw(RawFormat::VarianRaw) => crate::formats::varian::open_resolved_acquisition(
                &self.primary,
                self.companion("procpar").ok_or_else(|| {
                    ReadError::incomplete(self.root.clone().into(), "required procpar is missing")
                })?,
                self.companion("sampling.sch").ok_or_else(|| {
                    ReadError::incomplete(
                        self.root.clone().into(),
                        "sampling.sch resolver path is missing",
                    )
                })?,
                limits,
                varian,
            ),
            Format::Raw(RawFormat::JeolDelta) => {
                crate::formats::jeol::require_experimental_opt_in(
                    experimental_vendor_semantics,
                    self.primary.clone().into(),
                )
                .map_err(|error| error.with_format(self.format))?;
                crate::formats::jeol::open_acquisition_declared(
                    &self.primary,
                    limits,
                    sampling,
                    cancellation,
                )
            }
            Format::Processed(_) => Err(ReadError::unrecognized(
                self.primary.clone().into(),
                "candidate is not a raw acquisition",
            )),
        }
    }

    fn public(&self) -> ReadCandidate {
        ReadCandidate::new(self.format, self.primary.clone())
    }
}

pub(crate) fn resolve_and_select(
    path: &Path,
    hint: Option<Format>,
    preference: ReadPreference,
) -> Result<ResolvedCandidate, ReadError> {
    let candidates = resolve_candidates(path)?;
    select(path, candidates, hint, preference)
}

pub(crate) fn resolve_raw_and_select(path: &Path) -> Result<ResolvedCandidate, ReadError> {
    let mut candidates = resolve_candidates(path)?;
    candidates.retain(|candidate| candidate.format.kind() == crate::DatasetKind::Raw);
    retain_complete_if_any(&mut candidates);
    choose_same_kind(path, candidates).map_err(|error| {
        if error.kind() == crate::ReadErrorKind::Unrecognized {
            ReadError::unrecognized(path.into(), "no supported raw acquisition found")
        } else {
            error
        }
    })
}

fn resolve_candidates(path: &Path) -> Result<Vec<ResolvedCandidate>, ReadError> {
    fs::metadata(path).map_err(|error| ReadError::io(path, error))?;
    let mut candidates = resolve(path)?;
    candidates.sort_by(|a, b| (a.format, &a.primary).cmp(&(b.format, &b.primary)));
    candidates.dedup_by(|a, b| a.format == b.format && a.primary == b.primary);
    if candidates.is_empty() {
        return Err(ReadError::unrecognized(
            path.into(),
            "no supported dataset found",
        ));
    }
    Ok(candidates)
}

fn select(
    path: &Path,
    mut candidates: Vec<ResolvedCandidate>,
    hint: Option<Format>,
    preference: ReadPreference,
) -> Result<ResolvedCandidate, ReadError> {
    if let Some(expected) = hint {
        let actual = candidates.iter().map(ResolvedCandidate::public).collect();
        candidates.retain(|candidate| candidate.format == expected);
        retain_complete_if_any(&mut candidates);
        return choose_same_kind(path, candidates).map_err(|error| {
            if error.kind() == crate::ReadErrorKind::Unrecognized {
                ReadError::format_mismatch(path.into(), expected, actual)
            } else {
                error
            }
        });
    }

    retain_complete_if_any(&mut candidates);

    let raw: Vec<_> = candidates
        .iter()
        .filter(|value| value.format.kind() == crate::DatasetKind::Raw)
        .cloned()
        .collect();
    let processed: Vec<_> = candidates
        .iter()
        .filter(|value| value.format.kind() == crate::DatasetKind::Processed)
        .cloned()
        .collect();
    match (raw.is_empty(), processed.is_empty(), preference) {
        (false, false, ReadPreference::RequireUnique) => ambiguous(path, candidates),
        (false, false, ReadPreference::PreferRaw) => choose_same_kind(path, raw),
        (false, false, ReadPreference::PreferProcessed) => choose_same_kind(path, processed),
        (false, true, _) => choose_same_kind(path, raw),
        (true, false, _) => choose_same_kind(path, processed),
        (true, true, _) => unreachable!(),
    }
}

fn retain_complete_if_any(candidates: &mut Vec<ResolvedCandidate>) {
    if candidates
        .iter()
        .any(ResolvedCandidate::is_structurally_complete)
    {
        candidates.retain(ResolvedCandidate::is_structurally_complete);
    }
}

fn choose_same_kind(
    path: &Path,
    mut candidates: Vec<ResolvedCandidate>,
) -> Result<ResolvedCandidate, ReadError> {
    match candidates.len() {
        0 => Err(ReadError::unrecognized(
            path.into(),
            "asserted format was not found",
        )),
        1 => Ok(candidates.remove(0)),
        _ => ambiguous(path, candidates),
    }
}

fn ambiguous(
    path: &Path,
    candidates: Vec<ResolvedCandidate>,
) -> Result<ResolvedCandidate, ReadError> {
    let common_format = candidates
        .first()
        .map(|candidate| candidate.format)
        .filter(|format| {
            candidates
                .iter()
                .all(|candidate| candidate.format == *format)
        });
    let error = ReadError::ambiguous_candidates(
        path.into(),
        "more than one dataset candidate matches",
        candidates.iter().map(ResolvedCandidate::public).collect(),
    );
    Err(match common_format {
        Some(format) => error.with_format(format),
        None => error,
    })
}

fn resolve(path: &Path) -> Result<Vec<ResolvedCandidate>, ReadError> {
    if path.is_file() {
        return resolve_file(path);
    }
    let mut values = Vec::new();
    resolve_bruker_directory(path, &mut values)?;
    resolve_varian_directory(path, &mut values);
    resolve_standalone_directory(path, &mut values)?;
    Ok(values)
}

fn resolve_file(path: &Path) -> Result<Vec<ResolvedCandidate>, ReadError> {
    let name = lowercase_name(path);
    if matches!(name.as_str(), "fid" | "ser" | "acqus" | "acqu2s" | "acqu3s") {
        let root = path.parent().unwrap_or(path).to_path_buf();
        if name == "fid" && root.join("procpar").is_file() && !root.join("acqus").is_file() {
            return Ok(vec![varian(root)]);
        }
        let series = name == "ser"
            || matches!(name.as_str(), "acqu2s" | "acqu3s")
            || name == "acqus" && root.join("ser").is_file();
        return Ok(vec![bruker_raw(root, series)]);
    }
    if matches!(name.as_str(), "1r" | "1i") {
        let root = path.parent().unwrap_or(path).to_path_buf();
        return Ok(vec![bruker_processed(root, false)]);
    }
    if matches!(name.as_str(), "2rr" | "2ri" | "2ir" | "2ii" | "proc2s") {
        let root = path.parent().unwrap_or(path).to_path_buf();
        return Ok(vec![bruker_processed(root, true)]);
    }
    if name == "procs" {
        let root = path.parent().unwrap_or(path).to_path_buf();
        return Ok(bruker_processed_candidates(root));
    }
    if name == "procpar" || name == "sampling.sch" {
        let root = path.parent().unwrap_or(path).to_path_buf();
        return Ok(vec![varian(root)]);
    }
    if crate::formats::jeol::is_dataset(path) {
        let format = if crate::formats::jeol::is_frequency_domain(path)? {
            Format::Processed(processed::Format::JeolDelta)
        } else {
            Format::Raw(RawFormat::JeolDelta)
        };
        return Ok(vec![ResolvedCandidate {
            format,
            root: path.parent().unwrap_or(path).to_path_buf(),
            primary: path.to_path_buf(),
            companions: Vec::new(),
            optional_companions: Vec::new(),
        }]);
    }
    if is_jcamp(path)? {
        return Ok(vec![ResolvedCandidate {
            format: Format::Processed(processed::Format::JcampDx),
            root: path.parent().unwrap_or(path).to_path_buf(),
            primary: path.to_path_buf(),
            companions: Vec::new(),
            optional_companions: Vec::new(),
        }]);
    }
    Ok(Vec::new())
}

fn resolve_bruker_directory(
    path: &Path,
    values: &mut Vec<ResolvedCandidate>,
) -> Result<(), ReadError> {
    if lowercase_name(path) == "pdata" {
        append_procno(path, values)?;
        return Ok(());
    }
    if ["procs", "proc2s", "1r", "1i", "2rr", "2ri", "2ir", "2ii"]
        .into_iter()
        .any(|name| path.join(name).is_file())
    {
        values.extend(bruker_processed_candidates(path.to_path_buf()));
        return Ok(());
    }
    if path.join("ser").is_file()
        || path.join("acqus").is_file()
        || path.join("fid").is_file() && !path.join("procpar").is_file()
    {
        values.push(bruker_raw(path.to_path_buf(), path.join("ser").is_file()));
    }
    if path.join("pdata").is_dir() {
        append_procno(&path.join("pdata"), values)?;
    }
    Ok(())
}

fn append_procno(pdata: &Path, values: &mut Vec<ResolvedCandidate>) -> Result<(), ReadError> {
    let mut entries: Vec<_> = fs::read_dir(pdata)
        .map_err(|error| ReadError::io(pdata, error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && ["procs", "proc2s", "1r", "1i", "2rr", "2ri", "2ir", "2ii"]
                    .into_iter()
                    .any(|name| path.join(name).is_file())
        })
        .collect();
    entries.sort();
    values.extend(entries.into_iter().flat_map(bruker_processed_candidates));
    Ok(())
}

fn bruker_raw(root: PathBuf, series: bool) -> ResolvedCandidate {
    let primary = root.join(if series { "ser" } else { "fid" });
    let mut companions = vec![(root.join("acqus"), "acqus")];
    if series {
        companions.push((root.join("acqu2s"), "acqu2s"));
    }
    ResolvedCandidate {
        format: Format::Raw(RawFormat::BrukerRaw),
        optional_companions: vec![
            (root.join("acqu3s"), "acqu3s"),
            (root.join("nuslist"), "nuslist"),
        ],
        root,
        primary,
        companions,
    }
}

fn bruker_processed_candidates(root: PathBuf) -> Vec<ResolvedCandidate> {
    let complete_1d = root.join("1r").is_file() && root.join("procs").is_file();
    let complete_2d =
        root.join("2rr").is_file() && root.join("procs").is_file() && root.join("proc2s").is_file();
    if complete_1d && complete_2d {
        return vec![
            bruker_processed(root.clone(), false),
            bruker_processed(root, true),
        ];
    }
    if complete_2d {
        return vec![bruker_processed(root, true)];
    }
    if complete_1d {
        return vec![bruker_processed(root, false)];
    }

    let one_d_evidence = root.join("1r").is_file() || root.join("1i").is_file();
    let two_d_evidence = ["2rr", "2ri", "2ir", "2ii", "proc2s"]
        .into_iter()
        .any(|name| root.join(name).is_file());
    match (one_d_evidence, two_d_evidence) {
        (true, true) => vec![
            bruker_processed(root.clone(), false),
            bruker_processed(root, true),
        ],
        (false, true) => vec![bruker_processed(root, true)],
        _ => vec![bruker_processed(root, false)],
    }
}

fn bruker_processed(root: PathBuf, two_d: bool) -> ResolvedCandidate {
    let primary = root.join(if two_d { "2rr" } else { "1r" });
    let mut companions = vec![(root.join("procs"), "procs")];
    if two_d {
        companions.push((root.join("proc2s"), "proc2s"));
    }
    ResolvedCandidate {
        format: Format::Processed(processed::Format::BrukerTopSpin),
        optional_companions: if two_d {
            vec![
                (root.join("2ri"), "2ri"),
                (root.join("2ir"), "2ir"),
                (root.join("2ii"), "2ii"),
            ]
        } else {
            vec![(root.join("1i"), "1i")]
        },
        root,
        primary,
        companions,
    }
}

fn resolve_varian_directory(path: &Path, values: &mut Vec<ResolvedCandidate>) {
    if path.join("fid").is_file()
        && path.join("procpar").is_file()
        && (path
            .extension()
            .is_some_and(|value| value.eq_ignore_ascii_case("fid"))
            || !path.join("acqus").exists())
    {
        values.push(varian(path.to_path_buf()));
    }
}

fn varian(root: PathBuf) -> ResolvedCandidate {
    ResolvedCandidate {
        format: Format::Raw(RawFormat::VarianRaw),
        primary: root.join("fid"),
        companions: vec![(root.join("procpar"), "procpar")],
        optional_companions: vec![(root.join("sampling.sch"), "sampling.sch")],
        root,
    }
}

fn resolve_standalone_directory(
    path: &Path,
    values: &mut Vec<ResolvedCandidate>,
) -> Result<(), ReadError> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|error| ReadError::io(path, error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    entries.sort();
    for entry in entries {
        values.extend(resolve_file(&entry)?);
    }
    Ok(())
}

fn is_jcamp(path: &Path) -> Result<bool, ReadError> {
    let mut file = fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
    let mut bytes = [0u8; 65_536];
    let read = file
        .read(&mut bytes)
        .map_err(|error| ReadError::io(path, error))?;
    let text = String::from_utf8_lossy(&bytes[..read]).to_ascii_uppercase();
    Ok(text.contains("##JCAMP-DX=") && text.contains("##DATA TYPE="))
}

fn lowercase_name(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}
