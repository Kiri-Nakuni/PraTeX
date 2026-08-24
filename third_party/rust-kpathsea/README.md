# Vendored Rust Kpathsea boundary

This directory contains a focused fork of the Rust wrappers only. It does not
contain Kpathsea's C source or a compiled library.

## Provenance

Captured on 2026-08-24 from <https://github.com/dginev/rust-kpathsea>:

| crate | crates.io version | VCS source commit | crates.io archive SHA-256 |
|---|---:|---|---|
| `kpathsea` | 0.3.4 | `4d3ddfdca85886168a6f1d20802ac1ce6284a96b` | `c573f825f32403aef75bbd955c3427d55e8230e40f0d9b0a98330637b5c8fe1f` |
| `kpathsea_sys` | 0.2.3 | `555eba333bc0fcfbd1d5f584c203acfab8fc5dc6` | `72d72f7d17fa1de89f3fd72ca949733f937ef3ac37a1ba98d36fe09d8f9a0074` |

The local API/ownership patch is recorded verbatim as
`UPSTREAM-PATCH.patch`. Its source commit is
`998dd0dd588174f851074a7ec195cfb9495adaee` (API/ownership commit
`10b7a86e7545a8884047f0ac1b359377dd258c3d` followed by the locked-feature
cleanup) and its SHA-256 is
`3046d1f96db8115e766dda7ed82f71e2b2b786b664834a2b4442eb5c62b5ce75`.
The patch adds the explicit program-name and exact native-path APIs, typed
unlinked/encoding errors, Unix result-buffer release, complete typed format
constants, and the `in-process-only-caller` feature boundary. It retains the
published legacy subprocess API for other callers, but PraTeX does not enable
or call it.

Only package source needed to reproduce the wrapper build is vendored. Cargo
locks, registry marker files, examples, and upstream tests are intentionally
excluded. The patch artifact preserves the complete review diff, including
the tests which established the boundary before this snapshot was copied.

## License and unsafe audit

Both Rust crates declare `MIT OR Apache-2.0`; copies of both license texts are
included in each crate directory. Kpathsea itself is LGPL-2.1-or-later and is
not vendored here. The default Linux build fetches official TeX Live 2026
commit `fb6158926661cb7a7246b3a94a0cb170a9624d5a` (`svn78399`), or uses an
exact `KPATHSEA_SRC_DIR`, and links Kpathsea 6.4.2 statically. Building or
distributing that binary has source/relink obligations; see
`docs/kpathsea-port-notes.md`.

The vendored wrapper necessarily contains `unsafe` at its audited FFI edge:

- generated raw declarations in `kpathsea_sys`;
- creation, use, and destruction of the opaque Kpathsea handle;
- conversion of C result buffers to owned `PathBuf` values;
- `free` of those result buffers on Unix, in the same process/CRT;
- `Send` for the owned, non-`Sync` handle.

PraTeX keeps this dependency to Linux builds and owns one handle in a
single-threaded run-local resolver. Windows linking is deliberately disabled
until an allocator-matching release API is measured; other Unix targets remain
on PraTeX's safe resolver until their ABI is audited. PraTeX production `src/`
contains no `unsafe` for this integration.

## Consumer feature contract

PraTeX depends on this fork with `default-features = false`. The default Linux
feature `bundled-kpathsea` explicitly enables `in-process-only-caller` and
`build-from-source`; both include `system-probe`. The resolved tree must contain
those names while the subprocess backend remains absent. The following
features must never be unified into the production graph:

- `kpathsea/default`
- `kpathsea/subprocess-backend`
- `kpathsea_sys/default`

Run `tools/check-kpathsea-features.ps1` to inspect Cargo's resolved Linux
feature tree. `build-from-source` resolves `which`, `pkg-config`, and `cc` and
requires the pinned source through git or `KPATHSEA_SRC_DIR`. A fresh offline
machine therefore needs both a verified Cargo vendor source for those
transitive packages and the exact Kpathsea source tree; disabling the feature
is not an equivalent production build. Distro builders may opt out of default
features and enable `stats,system-kpathsea` to link a separately supplied
Kpathsea 6.4.2 library instead.
