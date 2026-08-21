//! PDF font資材のclean-room parserと解決境界。
//!
//! 各parserはI/OやPostScript実行をせず、呼び出し側から渡されたbyte列だけを読む。

pub(crate) mod afm;
pub(crate) mod encoding;
pub(crate) mod loader;
pub(crate) mod map;
pub(crate) mod type1;
