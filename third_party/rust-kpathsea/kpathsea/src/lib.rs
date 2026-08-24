#![deny(missing_docs)]
//! High-level Rust API for working with the kpathsea file-searching library for TeX
//!
//! Two backends are provided:
//!
//! * **in-process** — FFI calls into the system `libkpathsea` (the fast
//!   path, microseconds per lookup). Selected automatically when the
//!   library was found at build time: pkg-config or the
//!   `KPATHSEA_LIB_DIR` override on Unix, TeX Live's own kpathsea DLL
//!   (found next to `kpsewhich.exe`) on Windows — see `kpathsea_sys`'s
//!   build script. `KPATHSEA_NO_LINK=1` at build time forces the
//!   subprocess backend even when a library is available.
//! * **subprocess** — delegates to the host TeX distribution's own
//!   `kpsewhich` executable, fronted by a one-shot cache of the TeX
//!   tree's `ls-R` databases. Selected automatically when `libkpathsea`
//!   was *not* found at build time (e.g. MacTeX/BasicTeX ship no library
//!   at all), or explicitly via [`Kpaths::new_subprocess`]. Because it
//!   asks the host's resolver binary, it stays in sync with the ambient
//!   distribution by construction — including MiKTeX, which reimplements
//!   kpathsea. (This mirrors how Perl LaTeXML has always resolved TeX
//!   files; see `src/subprocess.rs`.)
//!
//! Latency profile (measured on a full TeX Live): in-process lookups are
//! tens of µs, hit or miss. Subprocess lookups are *bimodal*: sub-µs on an
//! `ls-R` cache hit — faster than the FFI path — but a cache miss costs a
//! `kpsewhich` spawn (tens to hundreds of ms, memoized process-wide per
//! executable so a repeated miss is only ever paid once, regardless of
//! which instance or thread asks).

#[cfg(kpathsea_linked)]
use kpathsea_sys::*;
#[cfg(any(all(kpathsea_linked, unix), test))]
use std::ffi::CStr;
#[cfg(any(kpathsea_linked, test))]
use std::ffi::CString;
use std::ffi::OsStr;
#[cfg(any(kpathsea_linked, feature = "subprocess-backend"))]
use std::ffi::OsString;
#[cfg(kpathsea_linked)]
use std::path::Path;
use std::path::PathBuf;

#[cfg(feature = "subprocess-backend")]
mod subprocess;
#[cfg(feature = "subprocess-backend")]
use subprocess::SubprocessKpse;

/// External result type for handling library errors
pub type Result<T> = std::result::Result<T, &'static str>;

/// Errors from APIs which preserve native operating-system paths.
///
/// The older string-returning methods retain their original `Option<String>`
/// surface. New exact-path methods use this typed error so an in-process-only
/// caller can distinguish an unavailable library from a filename which cannot
/// be represented by the linked platform ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PathError {
  /// This build did not link `libkpathsea`.
  InProcessUnavailable,
  /// A program or logical filename contains an interior NUL byte.
  InteriorNul,
  /// An operating-system filename cannot be represented without loss by this
  /// platform's narrow-character Kpathsea ABI.
  UnsupportedPathEncoding,
  /// `kpathsea_new` returned a null instance pointer.
  InitializationFailed,
}

impl std::fmt::Display for PathError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let message = match self {
      Self::InProcessUnavailable => "kpathsea: in-process libkpathsea is unavailable",
      Self::InteriorNul => "kpathsea: a path or program name contains an interior NUL byte",
      Self::UnsupportedPathEncoding => {
        "kpathsea: the platform path cannot be represented by the Kpathsea ABI"
      }
      Self::InitializationFailed => "kpathsea: libkpathsea initialization failed",
    };
    formatter.write_str(message)
  }
}

impl std::error::Error for PathError {}

#[cfg(kpathsea_linked)]
impl PathError {
  fn legacy_message(self) -> &'static str {
    match self {
      Self::InProcessUnavailable => "kpathsea: in-process libkpathsea is unavailable",
      Self::InteriorNul => "kpathsea: path contains a NUL byte",
      Self::UnsupportedPathEncoding => "kpathsea: path encoding is unsupported",
      Self::InitializationFailed => "kpathsea: libkpathsea initialization failed",
    }
  }
}

/// Result type for exact operating-system path APIs.
pub type PathResult<T> = std::result::Result<T, PathError>;

/// Stable error returned by in-process-only construction in an unlinked
/// build. No subprocess resolver is constructed or inspected on this path.
pub const IN_PROCESS_UNAVAILABLE: PathError = PathError::InProcessUnavailable;

/// The one unrecoverable configuration: no linked `libkpathsea` to call and no
/// `kpsewhich` executable to spawn, so nothing can ever be resolved.
#[cfg(not(kpathsea_linked))]
const NO_BACKEND: &str = "kpathsea: no libkpathsea is linked and no `kpsewhich` executable is \
                          available — TeX file lookups cannot resolve. Install a TeX distribution, \
                          or point KPSEWHICH at its kpsewhich.";

/// Kpathsea file-format type, for callers of
/// [`Kpaths::find_file_with_format`] that want to pass a known format.
///
/// Values mirror the C `kpse_file_format_type` enum; the common ones are
/// named in [`formats`], and any other enum value is passed through
/// faithfully. Owned by this crate — not re-exported from `kpathsea_sys` —
/// so the API is identical whether or not `libkpathsea` was linked (the
/// `kpathsea_sys` surface only exists in linked builds).
pub type Format = u32;

/// Common kpathsea format constants. Values are the C
/// `kpse_file_format_type` enum's (drift-checked against `kpathsea_sys` in
/// linked test builds); other enum values can be passed as plain [`Format`]
/// numbers.
pub mod formats {
  use super::Format;
  /// TeX font metrics (`.tfm`).
  pub const TFM: Format = 3;
  /// Adobe font metrics (`.afm`).
  pub const AFM: Format = 4;
  /// Dumped TeX formats (`.fmt`).
  pub const FMT: Format = 10;
  /// `.tex`, `.sty`, `.cls`, `.def`, `.ltx` and related source formats.
  pub const TEX: Format = 26;
  /// `.bib` bibliography source
  pub const BIB: Format = 6;
  /// `.bst` bibliography style
  pub const BST: Format = 7;
  /// `.cnf` kpathsea config
  pub const CNF: Format = 8;
  /// Fontmap files
  pub const FONTMAP: Format = 11;
  /// Type 1 (`.pfa`/`.pfb`) fonts
  pub const TYPE1: Format = 32;
  /// Virtual fonts (`.vf`).
  pub const VF: Format = 33;
  /// TrueType fonts
  pub const TRUETYPE: Format = 36;
  /// Program-specific text files (`PRATEXINPUTS` and `$TEXMF/pratex//` when
  /// the explicit Kpathsea program name is `pratex`).
  pub const PROGRAM_TEXT: Format = 39;
  /// Font encoding vectors (`.enc`).
  pub const ENC: Format = 44;
}

/// The wrapper's format constants must stay in lockstep with the C enum.
/// (Only checkable where the bindgen bindings exist: linked, non-Windows.)
#[cfg(all(test, kpathsea_linked, not(windows)))]
mod format_drift {
  #[test]
  fn formats_match_libkpathsea() {
    use kpathsea_sys::*;
    assert_eq!(crate::formats::TFM, kpse_file_format_type_kpse_tfm_format);
    assert_eq!(crate::formats::AFM, kpse_file_format_type_kpse_afm_format);
    assert_eq!(crate::formats::FMT, kpse_file_format_type_kpse_fmt_format);
    assert_eq!(crate::formats::TEX, kpse_file_format_type_kpse_tex_format);
    assert_eq!(crate::formats::BIB, kpse_file_format_type_kpse_bib_format);
    assert_eq!(crate::formats::BST, kpse_file_format_type_kpse_bst_format);
    assert_eq!(crate::formats::CNF, kpse_file_format_type_kpse_cnf_format);
    assert_eq!(
      crate::formats::FONTMAP,
      kpse_file_format_type_kpse_fontmap_format
    );
    assert_eq!(
      crate::formats::TYPE1,
      kpse_file_format_type_kpse_type1_format
    );
    assert_eq!(crate::formats::VF, kpse_file_format_type_kpse_vf_format);
    assert_eq!(
      crate::formats::TRUETYPE,
      kpse_file_format_type_kpse_truetype_format
    );
    assert_eq!(
      crate::formats::PROGRAM_TEXT,
      kpse_file_format_type_kpse_program_text_format
    );
    assert_eq!(crate::formats::ENC, kpse_file_format_type_kpse_enc_format);
  }
}

/// The `kpsewhich --format=NAME` spelling for the constants in [`formats`],
/// used by the subprocess backend. Formats without a mapping fall back to a
/// plain lookup (kpsewhich then guesses from the suffix, like
/// [`Kpaths::find_file`]).
#[cfg(feature = "subprocess-backend")]
fn kpsewhich_format_name(format: Format) -> Option<&'static str> {
  match format {
    formats::TFM => Some("tfm"),
    formats::AFM => Some("afm"),
    formats::FMT => Some("fmt"),
    formats::TEX => Some("tex"),
    formats::BIB => Some("bib"),
    formats::BST => Some("bst"),
    formats::CNF => Some("cnf"),
    formats::FONTMAP => Some("map"),
    formats::TYPE1 => Some("type1 fonts"),
    formats::VF => Some("vf"),
    formats::TRUETYPE => Some("truetype fonts"),
    formats::PROGRAM_TEXT => Some("other text files"),
    formats::ENC => Some("enc files"),
    _ => None,
  }
}

enum Backend {
  #[cfg(kpathsea_linked)]
  InProcess(kpathsea),
  #[cfg(feature = "subprocess-backend")]
  Subprocess(SubprocessKpse),
  #[allow(dead_code)]
  #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
  Unavailable,
}

/// High-level interface struct for the kpathsea API
pub struct Kpaths(Backend);

// SAFETY (retained 0.3.4 contract): Kpaths uniquely owns its C instance and
// contains no borrowed Rust data; it is not Sync, so Send permits sequential
// ownership transfer rather than concurrent calls through one instance.
// Construction and destruction, which touch Kpathsea's process-global program
// state, are serialized by KPSE_GLOBAL_LOCK. This does not establish that
// concurrent calls through distinct instances are sound; the impl is retained
// to avoid an unrelated semver break and needs a separate upstream audit.
unsafe impl Send for Kpaths {}

/// Resolve the `kpsewhich` executable: the `KPSEWHICH` env var when set
/// (a bare name is looked up through PATH, an absolute path is taken as-is),
/// otherwise `kpsewhich` on PATH. Both backends anchor on this executable —
/// in-process as the program name handed to `kpathsea_set_program_name`,
/// subprocess as the resolver to invoke.
#[cfg(any(kpathsea_linked, feature = "subprocess-backend"))]
fn kpsewhich_executable() -> Result<PathBuf> {
  let name = std::env::var_os("KPSEWHICH").unwrap_or_else(|| OsString::from("kpsewhich"));
  which::which(&name).map_err(|_| "Error finding kpsewhich executable")
}

/// Convert an OS string to Kpathsea's narrow C ABI without replacement.
#[cfg(unix)]
#[cfg(any(kpathsea_linked, test))]
fn os_str_to_cstring(value: &OsStr) -> PathResult<CString> {
  use std::os::unix::ffi::OsStrExt;
  CString::new(value.as_bytes()).map_err(|_| PathError::InteriorNul)
}

/// The crate has not yet established a versioned TeX Live contract for the
/// Windows narrow-character filename encoding. ASCII is invariant across the
/// plausible encodings; every wider or unpaired value is rejected for now.
#[cfg(windows)]
#[cfg(any(kpathsea_linked, test))]
fn os_str_to_cstring(value: &OsStr) -> PathResult<CString> {
  let value = value.to_str().ok_or(PathError::UnsupportedPathEncoding)?;
  if !value.is_ascii() {
    return Err(PathError::UnsupportedPathEncoding);
  }
  CString::new(value.as_bytes()).map_err(|_| PathError::InteriorNul)
}

#[cfg(not(any(unix, windows)))]
#[cfg(any(kpathsea_linked, test))]
fn os_str_to_cstring(value: &OsStr) -> PathResult<CString> {
  let value = value.to_str().ok_or(PathError::UnsupportedPathEncoding)?;
  CString::new(value.as_bytes()).map_err(|_| PathError::InteriorNul)
}

#[cfg(unix)]
#[cfg(any(all(kpathsea_linked, unix), test))]
fn c_bytes_to_path(bytes: Vec<u8>) -> PathResult<PathBuf> {
  use std::os::unix::ffi::OsStringExt;
  Ok(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(windows)]
#[cfg(test)]
fn c_bytes_to_path(bytes: Vec<u8>) -> PathResult<PathBuf> {
  let path = String::from_utf8(bytes).map_err(|_| PathError::UnsupportedPathEncoding)?;
  if !path.is_ascii() {
    return Err(PathError::UnsupportedPathEncoding);
  }
  Ok(PathBuf::from(path))
}

#[cfg(not(any(unix, windows)))]
#[cfg(test)]
fn c_bytes_to_path(bytes: Vec<u8>) -> PathResult<PathBuf> {
  let path = String::from_utf8(bytes).map_err(|_| PathError::UnsupportedPathEncoding)?;
  Ok(PathBuf::from(path))
}

/// A path as an exact `CString`, for `kpathsea_set_program_name`.
#[cfg(kpathsea_linked)]
fn path_to_cstring(path: &Path) -> PathResult<CString> {
  os_str_to_cstring(path.as_os_str())
}

/// [`kpsewhich_executable`] as a `CString`, for `kpathsea_set_program_name`.
#[cfg(kpathsea_linked)]
fn get_kpsewhich_path() -> Result<CString> {
  path_to_cstring(&kpsewhich_executable()?).map_err(|_| "kpsewhich path is not representable")
}

/// The running executable's path as a `CString` — the second-choice anchor.
#[cfg(kpathsea_linked)]
fn current_exe_program_name() -> Result<CString> {
  let path = std::env::current_exe().map_err(|_| "current executable path is unavailable")?;
  path_to_cstring(&path).map_err(|_| "current executable path is not representable")
}

/// Exact variant of [`program_name_anchor`] for in-process-only callers.
/// Discovery failures degrade through the same tiers, but a path which was
/// found and cannot cross the platform ABI is reported rather than replaced.
#[cfg(kpathsea_linked)]
fn exact_program_name_anchor() -> PathResult<CString> {
  match kpsewhich_executable() {
    Ok(path) => path_to_cstring(&path),
    Err(_) => match std::env::current_exe() {
      Ok(path) => path_to_cstring(&path),
      Err(_) => Ok(CString::from(c"kpsewhich")),
    },
  }
}

/// The program name to anchor libkpathsea on, degrading but never failing:
/// `kpsewhich` (which also locates the TeX distribution) → the running
/// executable → a literal.
///
/// Refusing to initialize is worse than a degraded anchor: an uninitialized
/// libkpathsea returns `None` for every lookup and ignores `TEXINPUTS` &c,
/// which need no TeX distribution at all. So a linked [`Kpaths::new`] can
/// always succeed.
///
/// How much a degraded anchor still resolves is platform-dependent. On Unix it
/// only costs TeX-*distribution* discovery, and env-var search paths keep
/// working. On Windows the anchor also governs where `texmf.cnf` is looked for,
/// so an anchor outside the distribution finds no config and resolves nothing —
/// initialized but inert, which is still better than the `Err` this replaces.
///
/// Both sources are injected so every tier is testable — `current_exe()` does
/// not fail on a live system.
#[cfg(kpathsea_linked)]
fn program_name_anchor(
  kpsewhich: Result<CString>,
  current_exe: impl FnOnce() -> Result<CString>,
) -> CString {
  kpsewhich
    .or_else(|_| current_exe())
    .unwrap_or_else(|_| CString::from(c"kpsewhich"))
}

/// Every tier degrades without failing — what makes a linked
/// [`Kpaths::new`] infallible.
#[cfg(all(test, kpathsea_linked))]
mod anchor_tiers {
  use super::*;

  #[test]
  fn prefers_kpsewhich_and_leaves_current_exe_unevaluated() {
    let mut consulted = false;
    let got = program_name_anchor(Ok(CString::new("/usr/bin/kpsewhich").unwrap()), || {
      consulted = true;
      Ok(CString::new("/proc/self/exe").unwrap())
    });
    assert_eq!(got.to_str().unwrap(), "/usr/bin/kpsewhich");
    assert!(
      !consulted,
      "current_exe must not be consulted when kpsewhich resolves"
    );
  }

  #[test]
  fn degrades_to_current_exe_when_kpsewhich_is_unresolvable() {
    let got = program_name_anchor(Err("no kpsewhich"), || {
      Ok(CString::new("/proc/self/exe").unwrap())
    });
    assert_eq!(got.to_str().unwrap(), "/proc/self/exe");
  }

  #[test]
  fn degrades_to_a_literal_when_every_source_fails() {
    // Previously propagated `Err`, leaving libkpathsea uninitialized.
    let got = program_name_anchor(Err("no kpsewhich"), || Err("no current_exe"));
    assert_eq!(got.to_str().unwrap(), "kpsewhich");
  }
}

/// libkpathsea's `kpse_set_program_name` mutates process-global state:
/// static path buffers and the environment via `putenv`. Two threads
/// constructing `Kpaths` concurrently interleave those buffers and crash
/// libkpathsea ("Can't get directory of program name", with garbled paths —
/// observed under parallel `cargo test`). Construction and (defensively)
/// teardown are serialized behind this lock; lookups on an existing
/// instance are unaffected.
#[cfg(kpathsea_linked)]
static KPSE_GLOBAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Walk a NULL-terminated C array of suffix strings (the layout kpathsea
/// uses for `format_info.suffix` and `format_info.alt_suffix`; the array
/// pointer itself may be NULL when a format has no suffixes), returning
/// `true` when `filename` ends with one of them. The filename must be
/// strictly longer than the suffix: a bare extension with an empty stem
/// (e.g. `.sty`) matches nothing, so it falls through to the default
/// format instead.
///
/// # Safety
/// `list` must be NULL or point to a NULL-terminated array of valid
/// NUL-terminated C strings.
#[cfg(all(kpathsea_linked, not(windows)))]
unsafe fn filename_has_suffix_in(filename: &str, mut list: *mut const_string) -> bool {
  let filename = filename.as_bytes();
  while !list.is_null() && !unsafe { *list }.is_null() {
    let suffix = unsafe { CStr::from_ptr(*list) }.to_bytes();
    if filename.len() > suffix.len() && filename.ends_with(suffix) {
      return true;
    }
    list = unsafe { list.offset(1) };
  }
  false
}

/// For a given filename, try to guess the kpse format type from the file
/// extension by looking it up in the format info table. This is a simplified
/// version of the find_format function in kpsewhich.
#[cfg(all(kpathsea_linked, not(windows)))]
fn guess_format_from_filename(kpse: kpathsea, filename: &str) -> Format {
  if !filename.contains('.') {
    // no extension in filename, shorcircuit and default to tex
    return formats::TEX;
  }
  // We go through each format type
  for format_type in 0..kpse_file_format_type_kpse_last_format {
    let format_info: &mut kpse_format_info_type =
      unsafe { &mut (*kpse).format_info[format_type as usize] };
    if format_info.type_.is_null() {
      // If this format hasn't been initialized yet, initialize it now.
      // Otherwise, it won't have the list of suffixes initialized.
      unsafe {
        kpathsea_init_format(kpse, format_type as kpse_file_format_type);
      }
    }

    // Check the suffixes, then the alternate suffixes, for this format
    // type. If the filename ends with one of them, we've found our format.
    if unsafe { filename_has_suffix_in(filename, format_info.suffix) }
      || unsafe { filename_has_suffix_in(filename, format_info.alt_suffix) }
    {
      return format_type as Format;
    }
  }

  // If we don't find any matching suffixes, we guess that it's a tex file
  formats::TEX
}

/// libkpathsea's per-format suffix tables, dumped from a live library
/// (TeX Live 2025, `kpathsea_init_format` + `format_info[..].suffix` /
/// `.alt_suffix`) in C-walk order: ascending formats, suffix list before
/// alt-suffix list within each format. Used on Windows, where the bindings
/// are opaque-pointer-only and `format_info` cannot be walked (see
/// `kpathsea_sys/src/bindings_windows.rs`). Linked non-Windows test builds
/// compile it too, for the drift canary in [`suffix_table_drift`] — Linux
/// CI verifies this table against the linked library's own walk.
#[cfg(all(kpathsea_linked, any(windows, test)))]
#[rustfmt::skip]
const FORMAT_SUFFIXES: &[(Format, &str)] = &[
  (0, "gf"), (1, "pk"),
  (3, ".tfm"), (4, ".afm"), (5, ".base"), (6, ".bib"), (7, ".bst"),
  (8, ".cnf"), (9, "ls-R"), (9, "ls-r"), (10, ".fmt"), (11, ".map"),
  (12, ".mem"), (13, ".mf"), (14, ".pool"), (15, ".mft"), (16, ".mp"),
  (17, ".pool"), (19, ".ocp"), (20, ".ofm"), (20, ".tfm"), (21, ".opl"),
  (21, ".pl"), (22, ".otp"), (23, ".ovf"), (23, ".vf"), (24, ".ovp"),
  (24, ".vpl"), (25, ".eps"), (25, ".epsi"),
  (26, ".tex"), (26, ".sty"), (26, ".cls"), (26, ".fd"), (26, ".aux"),
  (26, ".bbl"), (26, ".def"), (26, ".clo"), (26, ".ldf"),
  (28, ".pool"), (29, ".dtx"), (29, ".ins"), (30, ".pro"),
  (32, ".pfa"), (32, ".pfb"), (33, ".vf"), (35, ".ist"),
  (36, ".ttf"), (36, ".ttc"), (36, ".TTF"), (36, ".TTC"), (36, ".dfont"),
  (37, ".t42"), (37, ".T42"), (42, ".web"), (42, ".ch"),
  (43, ".w"), (43, ".web"), (43, ".ch"), (44, ".enc"), (46, ".sfd"),
  (47, ".otf"), (47, ".OTF"), (49, ".lig"),
  (51, ".lua"), (51, ".luatex"), (51, ".luc"), (51, ".luctex"),
  (51, ".texlua"), (51, ".texluc"), (51, ".tlu"),
  (52, ".fea"), (53, ".cid"), (53, ".cidmap"),
  (54, ".mlbib"), (54, ".bib"), (55, ".mlbst"), (55, ".bst"),
  (56, ".dll"), (56, ".so"), (57, ".ris"), (58, ".bltxml"),
];

/// Windows variant of [`guess_format_from_filename`]: same walk, same
/// match rule (suffix shorter than the filename, `ends_with`), same
/// default — over [`FORMAT_SUFFIXES`] instead of the C `format_info`
/// structs the opaque Windows bindings cannot expose.
#[cfg(all(kpathsea_linked, windows))]
fn guess_format_from_filename(_kpse: kpathsea, filename: &str) -> Format {
  if !filename.contains('.') {
    return formats::TEX;
  }
  for &(format, suffix) in FORMAT_SUFFIXES {
    if filename.len() > suffix.len() && filename.ends_with(suffix) {
      return format;
    }
  }
  formats::TEX
}

/// [`FORMAT_SUFFIXES`] is the Windows backend's substitute for walking the
/// C `format_info` structs; this canary keeps it honest against the linked
/// library on the platforms that CAN walk them. If a TeX Live update
/// changes a suffix list, this fails on Linux CI and the table gets
/// regenerated.
#[cfg(all(test, kpathsea_linked, not(windows)))]
mod suffix_table_drift {
  use super::*;

  #[test]
  fn format_suffixes_match_libkpathsea() {
    let kpaths = Kpaths::new().expect("needs a TeX toolchain with libkpathsea");
    let kpse = match &kpaths.0 {
      Backend::InProcess(kpse) => *kpse,
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(_) => {
        panic!("linked build should construct the in-process backend")
      }
    };
    let mut live: Vec<(Format, String)> = Vec::new();
    for format_type in 0..kpse_file_format_type_kpse_last_format {
      unsafe { kpathsea_init_format(kpse, format_type) };
      let info = unsafe { &(*kpse).format_info[format_type as usize] };
      for &list in &[info.suffix, info.alt_suffix] {
        let mut entry = list;
        while !entry.is_null() && !unsafe { *entry }.is_null() {
          let suffix = unsafe { CStr::from_ptr(*entry) }
            .to_str()
            .unwrap()
            .to_string();
          live.push((format_type as Format, suffix));
          entry = unsafe { entry.offset(1) };
        }
      }
    }
    let table: Vec<(Format, String)> = FORMAT_SUFFIXES
      .iter()
      .map(|&(format, suffix)| (format, suffix.to_string()))
      .collect();
    assert_eq!(
      table, live,
      "FORMAT_SUFFIXES drifted from the linked libkpathsea — regenerate it from this walk"
    );
  }
}

/// Copy a caller-owned C path and release it before decoding the copied bytes.
/// This ordering makes the empty and encoding-error branches obey the same
/// ownership rule as the successful branch.
///
/// # Safety
///
/// `value` must be null or point to a valid NUL-terminated C string accepted
/// by `release`; `release` must be the allocator-matching deallocator for it.
#[cfg(any(all(kpathsea_linked, unix), test))]
unsafe fn copy_and_release_c_path(
  value: *mut std::os::raw::c_char,
  release: unsafe fn(*mut std::os::raw::c_char),
) -> PathResult<Option<PathBuf>> {
  if value.is_null() {
    return Ok(None);
  }
  let bytes = unsafe { CStr::from_ptr(value) }.to_bytes().to_vec();
  unsafe { release(value) };
  if bytes.is_empty() {
    Ok(None)
  } else {
    c_bytes_to_path(bytes).map(Some)
  }
}

#[cfg(all(kpathsea_linked, unix))]
unsafe fn release_kpathsea_path(value: *mut std::os::raw::c_char) {
  unsafe { kpathsea_sys::free(value.cast()) };
}

impl Kpaths {
  #[cfg(kpathsea_linked)]
  fn from_in_process_parts(anchor: CString, program_name: Option<CString>) -> PathResult<Self> {
    // Serialized: see KPSE_GLOBAL_LOCK. Construction, lookup, and destruction
    // of an instance should remain confined to its owning TeX run/thread. The
    // legacy Send impl is retained for compatibility, but this constructor
    // does not claim cross-thread program-name mutation is safe.
    let _guard = KPSE_GLOBAL_LOCK
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    let kpse = unsafe { kpathsea_new() };
    if kpse.is_null() {
      return Err(PathError::InitializationFailed);
    }
    let explicit = program_name
      .as_ref()
      .map_or(std::ptr::null(), |name| name.as_ptr());
    unsafe { kpathsea_set_program_name(kpse, anchor.as_ptr(), explicit) };
    Ok(Kpaths(Backend::InProcess(kpse)))
  }

  /// Obtain a new kpathsea struct, with metadata for the current rust executable.
  ///
  /// Selects the in-process `libkpathsea` backend when the library was
  /// linked at build time, and the subprocess-`kpsewhich` backend
  /// otherwise. Use [`Kpaths::is_in_process`] to inspect the choice.
  ///
  /// **On a linked build this never returns `Err`** — the program-name anchor
  /// degrades instead (see [`program_name_anchor`]). The `Result` remains for
  /// API stability and for the unlinked build, where the subprocess backend
  /// has nothing to shell out to without a `kpsewhich`.
  ///
  /// Construction itself is cheap (measured ~0.1ms, serialized
  /// process-wide on the in-process backend because
  /// `kpse_set_program_name` mutates global state). The expensive step on
  /// the in-process backend is each instance's FIRST lookup (~150ms on a
  /// full TeX Live): libkpathsea parses its config and builds a private
  /// in-memory copy of the `ls-R` database — tens of MB — per instance,
  /// whatever the format. Construct once and reuse — e.g. one instance
  /// per thread — rather than constructing (and re-warming) per lookup.
  /// The subprocess backend shares one `ls-R` cache process-wide and has
  /// no per-instance warm-up.
  pub fn new() -> Result<Self> {
    #[cfg(kpathsea_linked)]
    {
      // Prefer the `kpsewhich` location: kpathsea suggests our own executable
      // name, but that can miss the available TeX distribution.
      let program_name = program_name_anchor(get_kpsewhich_path(), current_exe_program_name);
      Self::from_in_process_parts(program_name, None).map_err(PathError::legacy_message)
    }
    #[cfg(all(not(kpathsea_linked), feature = "subprocess-backend"))]
    {
      Self::new_subprocess().map_err(|_| {
        // Terminal: neither backend exists, so NO file can ever resolve. Say so
        // on stderr as well as in the `Err` — callers routinely `.ok()` an error
        // away, and a silently inert resolver is indistinguishable from a TeX
        // tree that simply lacks the file.
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| eprintln!("{NO_BACKEND}"));
        NO_BACKEND
      })
    }
    #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
    {
      Err(NO_BACKEND)
    }
  }

  /// Construct an in-process resolver with an explicit Kpathsea program name.
  ///
  /// The second C argument (`argv0`) remains anchored on the discovered
  /// `kpsewhich` executable so Kpathsea selects that TeX distribution. The
  /// third C argument is exactly `program_name`, allowing callers such as
  /// PraTeX to select `TEXINPUTS.pratex` without pretending to be another TeX
  /// engine.
  ///
  /// Unlike [`Kpaths::new`], this method never constructs the subprocess
  /// backend. An unlinked build returns [`IN_PROCESS_UNAVAILABLE`] without
  /// locating, inspecting, or invoking `kpsewhich`. The instance must be
  /// constructed, queried, and dropped on one TeX run/thread. The crate's
  /// legacy `Send` implementation remains for compatibility; this API does not
  /// extend it into a guarantee about cross-thread program-name mutation.
  pub fn new_in_process_with_program_name(program_name: &str) -> PathResult<Self> {
    #[cfg(kpathsea_linked)]
    {
      let program_name = CString::new(program_name).map_err(|_| PathError::InteriorNul)?;
      let anchor = exact_program_name_anchor()?;
      Self::from_in_process_parts(anchor, Some(program_name))
    }
    #[cfg(not(kpathsea_linked))]
    {
      let _ = program_name;
      Err(IN_PROCESS_UNAVAILABLE)
    }
  }

  /// Obtain a kpathsea struct that always resolves through the host's
  /// `kpsewhich` executable (located via the `KPSEWHICH` env var or PATH),
  /// regardless of whether `libkpathsea` is linked. This is the resolution
  /// strategy Perl LaTeXML uses, and the only one possible on TeX
  /// distributions that ship no `libkpathsea` (e.g. MacTeX).
  #[cfg(feature = "subprocess-backend")]
  pub fn new_subprocess() -> Result<Self> {
    Ok(Kpaths(Backend::Subprocess(SubprocessKpse::new()?)))
  }

  /// Like [`Kpaths::new_subprocess`], with an explicit path to the
  /// `kpsewhich` executable (bypassing `KPSEWHICH`/PATH lookup). The path is
  /// not validated up front; a missing executable simply makes every lookup
  /// return `None`.
  #[cfg(feature = "subprocess-backend")]
  pub fn with_kpsewhich<P: Into<PathBuf>>(path: P) -> Self {
    Kpaths(Backend::Subprocess(SubprocessKpse::with_kpsewhich(
      path.into(),
    )))
  }

  /// `true` when this instance calls `libkpathsea` in-process, `false`
  /// when it shells out to `kpsewhich`. Useful for callers that gate
  /// per-lookup work (e.g. format-table prewarming) on the lookup cost.
  pub fn is_in_process(&self) -> bool {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(_) => true,
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(_) => false,
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => false,
    }
  }

  /// Find a file base name, auto-completing with the standard TeX extensions if needed
  pub fn find_file(&self, name: &str) -> Option<String> {
    #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
    let _ = name;
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        let file_format_type = guess_format_from_filename(*kpse, name);
        self.find_file_with_format(name, file_format_type)
      }
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(sub) => sub.find_first(&[name]),
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => None,
    }
  }

  /// Search a list of candidate names, returning the first one found.
  ///
  /// With the subprocess backend this mirrors Perl LaTeXML's
  /// `pathname_kpsewhich`: the `ls-R` cache is consulted for each candidate
  /// first, and a full miss costs only ONE `kpsewhich` invocation for the
  /// whole list. With the in-process backend it is a `find_file` loop.
  pub fn find_first(&self, candidates: &[&str]) -> Option<String> {
    #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
    let _ = candidates;
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(_) => candidates.iter().find_map(|c| self.find_file(c)),
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(sub) => sub.find_first(candidates),
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => None,
    }
  }

  /// Find a file with a caller-supplied format, bypassing `guess_format_from_filename`.
  ///
  /// `guess_format_from_filename` walks every format type in the kpathsea format
  /// info table and lazily initializes each one (via `kpathsea_init_format`)
  /// before comparing suffixes — measured at ~15-20ms of one-time work on a
  /// fresh `Kpaths` instance. (The bulk of a first in-process lookup, ~150ms
  /// on a full TeX Live, is libkpathsea building its private in-memory `ls-R`
  /// db, which every first search pays regardless of format — see
  /// [`Kpaths::new`].) Prefer this method when you already know the kpathsea
  /// format — it issues exactly one `kpathsea_find_file` call with no
  /// format-table walk.
  ///
  /// With the subprocess backend the `ls-R` cache is consulted first, like
  /// [`Kpaths::find_file`]; on a cache miss, formats from [`formats`] are
  /// passed as `kpsewhich --format=NAME`, and other format values fall back
  /// to a plain lookup.
  ///
  /// Names containing an interior NUL byte cannot exist in a TeX tree and
  /// resolve to `None`.
  pub fn find_file_with_format(&self, name: &str, format: Format) -> Option<String> {
    #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
    let _ = (name, format);
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(_) => self
        .find_file_path_with_format(OsStr::new(name), format, false)
        .ok()
        .flatten()
        .and_then(|path| path.into_os_string().into_string().ok()),
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(sub) => {
        sub.find_with_format_name(name, kpsewhich_format_name(format))
      }
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => None,
    }
  }

  /// Find a file while preserving the platform's exact path representation.
  ///
  /// `must_exist` is passed unchanged to `kpathsea_find_file`. On Unix both
  /// the logical name and returned path are arbitrary non-NUL byte strings;
  /// no UTF-8 conversion occurs. Windows currently returns
  /// [`PathError::UnsupportedPathEncoding`] before calling the FFI at all:
  /// TeX Live's DLL allocator cannot be paired with a Rust-side `free` until a
  /// matching ownership boundary is established and tested.
  ///
  /// The subprocess backend remains available to the legacy String-returning
  /// methods. It cannot satisfy this method's exact native-path contract,
  /// because its cache and stdout parser are String-based, so this method
  /// returns [`PathError::UnsupportedPathEncoding`] without spawning it.
  pub fn find_file_path_with_format(
    &self,
    name: &OsStr,
    format: Format,
    must_exist: bool,
  ) -> PathResult<Option<PathBuf>> {
    #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
    let _ = (name, format, must_exist);
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        #[cfg(unix)]
        {
          let c_name = os_str_to_cstring(name)?;
          let c_filename_buf =
            unsafe { kpathsea_find_file(*kpse, c_name.as_ptr(), format, i32::from(must_exist)) };
          unsafe { copy_and_release_c_path(c_filename_buf, release_kpathsea_path) }
        }
        #[cfg(not(unix))]
        {
          let _ = (kpse, name, format, must_exist);
          Err(PathError::UnsupportedPathEncoding)
        }
      }
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(sub) => {
        let _ = (sub, name, format, must_exist);
        Err(PathError::UnsupportedPathEncoding)
      }
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => Err(IN_PROCESS_UNAVAILABLE),
    }
  }
}

#[cfg(test)]
mod exact_path_unit_tests {
  use super::*;
  use std::sync::Mutex;
  use std::sync::atomic::{AtomicUsize, Ordering};

  static RELEASE_TEST_LOCK: Mutex<()> = Mutex::new(());
  static RELEASE_COUNT: AtomicUsize = AtomicUsize::new(0);

  unsafe fn reclaim_test_cstring(value: *mut std::os::raw::c_char) {
    RELEASE_COUNT.fetch_add(1, Ordering::SeqCst);
    drop(unsafe { CString::from_raw(value) });
  }

  #[test]
  fn null_c_result_is_a_miss_and_is_not_freed() {
    let _guard = RELEASE_TEST_LOCK.lock().unwrap();
    let before = RELEASE_COUNT.load(Ordering::SeqCst);
    let result = unsafe { copy_and_release_c_path(std::ptr::null_mut(), reclaim_test_cstring) };
    assert_eq!(result, Ok(None));
    assert_eq!(RELEASE_COUNT.load(Ordering::SeqCst), before);
  }

  #[test]
  fn empty_c_result_is_freed_before_becoming_a_miss() {
    let _guard = RELEASE_TEST_LOCK.lock().unwrap();
    let before = RELEASE_COUNT.load(Ordering::SeqCst);
    let value = CString::new(Vec::<u8>::new()).unwrap().into_raw();
    let result = unsafe { copy_and_release_c_path(value, reclaim_test_cstring) };
    assert_eq!(result, Ok(None));
    assert_eq!(RELEASE_COUNT.load(Ordering::SeqCst), before + 1);
  }

  #[test]
  fn interior_nul_is_a_typed_error() {
    assert_eq!(
      os_str_to_cstring(OsStr::new("bad\0name")),
      Err(PathError::InteriorNul)
    );
  }

  #[test]
  fn repeated_successful_copies_release_every_result() {
    let _guard = RELEASE_TEST_LOCK.lock().unwrap();
    let before = RELEASE_COUNT.load(Ordering::SeqCst);
    for index in 0..1024 {
      let value = CString::new(format!("/tmp/result-{index}.tex"))
        .unwrap()
        .into_raw();
      let result = unsafe { copy_and_release_c_path(value, reclaim_test_cstring) }
        .unwrap()
        .unwrap();
      assert!(result.ends_with(format!("result-{index}.tex")));
    }
    assert_eq!(RELEASE_COUNT.load(Ordering::SeqCst), before + 1024);
  }

  #[cfg(unix)]
  #[test]
  fn unix_c_path_preserves_invalid_utf8_directory_bytes() {
    use std::os::unix::ffi::OsStrExt;

    let _guard = RELEASE_TEST_LOCK.lock().unwrap();
    let bytes = b"/tmp/kpathsea-\xff/file.tex".to_vec();
    let value = CString::new(bytes.clone()).unwrap().into_raw();
    let result = unsafe { copy_and_release_c_path(value, reclaim_test_cstring) }
      .unwrap()
      .unwrap();
    assert_eq!(result.as_os_str().as_bytes(), bytes);
  }

  #[cfg(windows)]
  #[test]
  fn windows_unpaired_surrogate_is_a_typed_encoding_error() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let value = OsString::from_wide(&[0xd800]);
    assert_eq!(
      os_str_to_cstring(&value),
      Err(PathError::UnsupportedPathEncoding)
    );
  }

  #[cfg(windows)]
  #[test]
  fn windows_invalid_c_bytes_are_freed_before_encoding_error() {
    let _guard = RELEASE_TEST_LOCK.lock().unwrap();
    let before = RELEASE_COUNT.load(Ordering::SeqCst);
    let value = CString::new(vec![0xff]).unwrap().into_raw();
    let result = unsafe { copy_and_release_c_path(value, reclaim_test_cstring) };
    assert_eq!(result, Err(PathError::UnsupportedPathEncoding));
    assert_eq!(RELEASE_COUNT.load(Ordering::SeqCst), before + 1);
  }
}

impl Drop for Kpaths {
  /// Cleanup the kpathsea pointer in the destructor
  fn drop(&mut self) {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        // Serialized: see KPSE_GLOBAL_LOCK.
        let _guard = KPSE_GLOBAL_LOCK
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe { kpathsea_finish(*kpse) }
      }
      #[cfg(feature = "subprocess-backend")]
      Backend::Subprocess(_) => {}
      #[cfg(all(not(kpathsea_linked), not(feature = "subprocess-backend")))]
      Backend::Unavailable => {}
    }
  }
}
