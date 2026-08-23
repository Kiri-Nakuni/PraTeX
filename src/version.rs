/// PraTeX 1 は、公開した機能一覧を満たした最初の完成版だけが名乗る。
///
/// `\pratexversion` は TeX の整数文脈で読める major gate であり、開発中は 0 を返す。
/// 小数点以下と末尾の零を含む版文字列は `\pratexrevision` と banner が担う。
pub(crate) const PRATEX_VERSION_MAJOR: i32 = 0;

pub(crate) const PRATEX_REVISION: &str = concat!(env!("CARGO_PKG_VERSION"), "-dev");

pub(crate) const BANNER: &str = concat!(
    "This is PraTeX, Version ",
    env!("CARGO_PKG_VERSION"),
    "-dev"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 完成条件より前に版一を名乗らない() {
        assert_eq!(PRATEX_VERSION_MAJOR, 0);
        assert_eq!(PRATEX_REVISION, "0.1.0-dev");
        assert_eq!(BANNER, "This is PraTeX, Version 0.1.0-dev");
    }
}
