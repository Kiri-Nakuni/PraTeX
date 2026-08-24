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
included in each crate directory. The optional system `libkpathsea` is an
external LGPL-2.1-or-later library and is not vendored here. Building or
distributing a static Kpathsea has separate source/relink obligations; see
`docs/kpathsea-port-notes.md`.

The vendored wrapper necessarily contains `unsafe` at its audited FFI edge:

- generated raw declarations in `kpathsea_sys`;
- creation, use, and destruction of the opaque Kpathsea handle;
- conversion of C result buffers to owned `PathBuf` values;
- `free` of those result buffers on Unix, in the same process/CRT;
- `Send` for the owned, non-`Sync` handle.

PraTeX keeps this dependency to Unix non-WASM builds and owns one handle in a
single-threaded run-local resolver. Windows linking is deliberately disabled
until an allocator-matching release API is measured. PraTeX production
`src/` contains no `unsafe` for this integration.

## Consumer feature contract

PraTeX depends on this fork with `default-features = false` and explicitly
enables only `in-process-only-caller`. That feature includes `system-probe`,
and the resolved tree must contain both names. The
following features must never be unified into the production graph:

- `kpathsea/default`
- `kpathsea/subprocess-backend`
- `kpathsea/build-from-source`
- `kpathsea_sys/default`
- `kpathsea_sys/build_from_source`

Run `tools/check-kpathsea-features.ps1` to inspect Cargo's resolved Linux
feature tree. `system-probe` resolves `which`, `pkg-config`, and `cc` even when
no system library is found. A fresh offline machine therefore needs a verified
Cargo vendor source for those transitive packages; disabling the probe is not
an equivalent production build.
