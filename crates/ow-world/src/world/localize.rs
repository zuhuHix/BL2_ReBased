//! AI-assisted (Codex/GPT-6.1 Sol). INT-only Object.Localize, from the independently authored own-words
//! NATIVE_CONTROLLER_HELPERS.md, Object.Localize. Native behavior remains UNVERIFIED against the running game.
//! No config merger or global language is inferred here: callers supply ordered roots and explicit options.
//! Like the existing item-card reader, files are UTF-16 (BOM selects endian, otherwise little endian), cached
//! on first use. Unlike that reader, I/O failures other than absence are errors. Unread selected syntax is NotYet.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ow_core::{DetMap, NotYet, PackageName, ParityMode};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

const KEY: &str = "Object.Localize(string,string,string)";
const RECORD: &str = "NATIVE_CONTROLLER_HELPERS.md (Object.Localize)";
// An implementation resource bound, not a recovered game limit. Larger files are refused, never truncated.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// An explicitly represented backend. Absence of this service is not the game's pre-initialization state.
pub struct Localization {
    ready: bool,
    roots: Vec<PathBuf>,
    files: DetMap<(usize, PackageName), Option<(PathBuf, String)>>,
}

impl Localization {
    pub fn not_ready() -> Self {
        Self {
            ready: false,
            roots: Vec::new(),
            files: DetMap::new(),
        }
    }

    /// Ordered roots, earlier to later priority. Only known INT / SHOWMISSINGLOC off is supported by this slice.
    pub fn ready(roots: Vec<PathBuf>, language: &str, show_missing: bool) -> Result<Self, VmError> {
        if !language.eq_ignore_ascii_case("INT") || show_missing {
            return Err(NotYet::not_ported(
                "Object.Localize language/options outside INT with SHOWMISSINGLOC off",
                RECORD,
            )
            .into());
        }
        Ok(Self {
            ready: true,
            roots,
            files: DetMap::new(),
        })
    }

    pub fn get(&mut self, section: &str, key: &str, package: &str) -> Result<String, VmError> {
        if !self.ready {
            return Ok(cut(key).to_string());
        }
        // Reuse the project's package identity validation before any filesystem lookup. The additional
        // filename checks refuse Windows-invalid spellings too, including wildcard and trailing-space aliases.
        let name = PackageName::parse(package)
            .map_err(|e| bad(format!("invalid package identity: {e}")))?;
        if package.contains(['<', '>', '"', '|', '?', '*']) || package.ends_with(' ') {
            return Err(bad("invalid package filename"));
        }
        let upper = package.to_ascii_uppercase();
        if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (upper.starts_with("COM") || upper.starts_with("LPT"))
                && upper.len() == 4
                && matches!(upper.as_bytes()[3], b'1'..=b'9')
        {
            return Err(bad("reserved package filename"));
        }
        for index in (0..self.roots.len()).rev() {
            let cache_key = (index, name.clone());
            if !self.files.contains_key(&cache_key) {
                let file = read_file(&self.roots[index], &name)?;
                self.files.insert(cache_key.clone(), file);
            }
            if let Some((path, text)) = self.files.get(&cache_key).and_then(Option::as_ref)
                && let Some(value) = selected(text, section, key).map_err(|e| context(path, e))?
            {
                return Ok(cut(&value).to_string());
            }
        }
        Ok(cut(&format!("?INT?{package}.{section}.{key}?")).to_string())
    }
}

fn cut(s: &str) -> &str {
    s.split_once("###").map_or(s, |(head, _)| head)
}

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script {
        function: KEY.to_string(),
        message: message.into(),
    }
}

fn context(path: &Path, e: VmError) -> VmError {
    match e {
        VmError::Script { message, .. } => bad(format!("{}: {message}", path.display())),
        other => other,
    }
}

/// Enumerating the directory avoids platform-dependent case-sensitive filename lookup and refuses two matches.
fn read_file(root: &Path, package: &PackageName) -> Result<Option<(PathBuf, String)>, VmError> {
    // The INT folder in its own case (Aspyr's Linux build has `int`; review r1 of #675): two that differ only in case
    // are an error.
    let dir = match ow_core::install::resolve_child(root, "INT") {
        Ok(Some(dir)) => dir,
        Ok(None) => return Ok(None),
        Err(e) => return Err(bad(e.to_string())),
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(bad(format!("{}: {e}", dir.display()))),
    };
    let filename = format!("{}.int", package.as_str());
    let mut found = None;
    for entry in entries {
        let entry = entry.map_err(|e| bad(format!("{}: {e}", dir.display())))?;
        if entry
            .file_name()
            .to_str()
            .is_some_and(|s| s.eq_ignore_ascii_case(&filename))
        {
            if found.is_some() {
                return Err(bad(format!(
                    "{}: ambiguous localization filename",
                    dir.display()
                )));
            }
            found = Some(entry.path());
        }
    }
    let Some(path) = found else { return Ok(None) };
    let read = || -> Result<String, VmError> {
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
            Err(e) => return Err(bad(e.to_string())),
        };
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| bad(e.to_string()))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(NotYet::not_ported(
                "Object.Localize file exceeds the 16 MiB reader bound",
                RECORD,
            )
            .into());
        }
        utf16(&bytes)
    };
    Ok(Some((path.clone(), read().map_err(|e| context(&path, e))?)))
}

fn utf16(bytes: &[u8]) -> Result<String, VmError> {
    let (big, body) = match bytes {
        [0xff, 0xfe, rest @ ..] => (false, rest),
        [0xfe, 0xff, rest @ ..] => (true, rest),
        rest => (false, rest),
    };
    if body.len() % 2 != 0 {
        return Err(bad("odd UTF-16 byte length"));
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|c| {
            if big {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    let text = String::from_utf16(&units).map_err(|e| bad(format!("invalid UTF-16: {e}")))?;
    if text.contains('\0') {
        return Err(bad("NUL in localization text"));
    }
    Ok(text)
}

/// Scan only the requested section/key. Unused escapes, array keys and unfamiliar lines do not invalidate a hit.
fn selected(text: &str, section: &str, key: &str) -> Result<Option<String>, VmError> {
    let mut inside = false;
    let mut found = None;
    for line in text.split(['\r', '\n']) {
        let line = line.trim();
        if line.starts_with(';') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                inside = name.trim().eq_ignore_ascii_case(section);
            } else {
                // A broken boundary cannot safely be treated as another key in the previous section.
                return Err(bad("malformed localization section boundary"));
            }
            continue;
        }
        if !inside {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            if line.eq_ignore_ascii_case(key) {
                return Err(bad("selected key has no '='"));
            }
            continue;
        };
        let name = name.trim();
        let array_base = name.split_once('[').map(|(base, _)| base.trim());
        if !name.eq_ignore_ascii_case(key)
            && !array_base.is_some_and(|b| b.eq_ignore_ascii_case(key))
        {
            continue;
        }
        if name.contains(['[', ']']) {
            return Err(NotYet::not_read("Object.Localize selected array key", RECORD).into());
        }
        if found.is_some() {
            return Err(bad("ambiguous duplicate localization key"));
        }
        let value = value.trim();
        let value = if value.starts_with('"') || value.ends_with('"') {
            value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .ok_or_else(|| bad("unmatched selected value quotes"))?
        } else {
            value
        };
        if value.contains('\\') || value.contains('"') {
            return Err(NotYet::not_read(
                "Object.Localize selected escape/embedded quote syntax",
                RECORD,
            )
            .into());
        }
        found = Some(value.to_string());
    }
    Ok(found)
}

pub(super) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity == ParityMode::Reference {
        return Ok(());
    }
    vm.register_native(
        KEY,
        Rc::new(|c: &mut NativeCall<'_>| {
            let mut args = Vec::new();
            for index in 0..3 {
                let Value::Str(s) = c.arg(index)? else {
                    return Err(VmError::BadArgument {
                        function: KEY.to_string(),
                        index,
                        reason: "expected string",
                    });
                };
                args.push(s.to_string());
            }
            let backend = c.vm.service::<Localization>().ok_or_else(|| {
                NotYet::layer_absent(
                    "Object.Localize backend/readiness/language/options",
                    "localization backend",
                    RECORD,
                )
            })?;
            let value = backend
                .try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("Localization"))?
                .get(&args[0], &args[1], &args[2])?;
            Ok(Value::string(&value))
        }),
    )
}

#[cfg(test)]
mod tests;
