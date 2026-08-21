//! PDF font資材のclean-room parserと解決境界。
//!
//! 各parserはI/OやPostScript実行をせず、呼び出し側から渡されたbyte列だけを読む。

mod afm;
mod encoding;
mod map;
mod type1;
