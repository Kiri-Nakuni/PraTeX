# From-source build config (`build_from_source` feature)

These are the **only** in-tree files needed to build a static libkpathsea from
source — and they are all **original** to this crate. The kpathsea C sources
themselves are **not** here (see Licensing below); `build.rs`
(`try_build_from_source`) fetches them at build time.

## Layout

Hand-written stand-ins for kpathsea's autoconf output (the `cc` build has no
`./configure` step). `build.rs` picks the per-OS `c-auto.h` and always adds
`common/`; revisit the headers on a `KPSE_REF` bump.

- `common/config.h` — one-line shim (`#include <kpathsea/config.h>`) for the few
  units that include bare `config.h` (autotools puts the generated header at the
  build root).
- `common/kpathsea/paths.h` — stub `DEFAULT_*` path strings; the host's
  `texmf.cnf` overrides all of them at runtime.
- `msvc/kpathsea/c-auto.h` — the MSVC/UCRT feature set (windows-msvc leg).
- `unix/kpathsea/c-auto.h` — the POSIX/glibc feature set (Unix leg; verified on
  Linux, best-effort elsewhere).

## Source acquisition (fetch, not vendor)

`build.rs` obtains the `texk/kpathsea` C sources from:
1. `KPATHSEA_SRC_DIR` if set (offline / pre-fetched builds), else
2. a sparse, shallow `git` fetch from the TeX Live source mirror at the pinned
   commit `KPSE_REF` (PraTeX pin =
   `fb6158926661cb7a7246b3a94a0cb170a9624d5a`, kpathsea **6.4.2 / TL2026**).

It then compiles the per-OS source set (`KPATHSEA_COMMON_SOURCES` plus the leg's
units, in `build.rs`) with these headers → a static libkpathsea → in-process,
self-contained link (on Windows, no runtime `kpathsealibw64.dll`). **Zero source
patches.**

## When to use it

The crate itself keeps `build_from_source` opt-in. PraTeX enables it in its
default Linux feature because the TUG TeX Live binary distribution does not
install a standalone library. This produces a binary pinned to exactly
`KPSE_REF`, independent of a development package. Distribution builders can
instead use the normal system probe (`pkg-config`, `KPATHSEA_LIB_DIR`, and
optional `KPATHSEA_STATIC=1`) through PraTeX's explicit `system-kpathsea`
feature. Windows/MSVC remains outside PraTeX's enabled dependency graph until
the returned-path allocator contract has been verified.

## Licensing (why the source is fetched, not bundled)

kpathsea is **LGPL-2.1**; this crate is **MIT OR Apache-2.0**. To keep the crate
free of LGPL-licensed files, the LGPL sources are fetched at build time rather
than committed here. Only the original config headers above ship in-tree.

Note that a binary which **statically links** the fetched libkpathsea contains
LGPL code and so carries LGPL §6 obligations (source availability + a relink
provision). The `build_from_source` feature is opt-in and off by default for
this crate, but PraTeX enables it by default on Linux. Downstreams that
distribute such binaries must satisfy §6; shipping only this crate and the
`KPSE_REF` pin is not by itself the required source/relink offer.
