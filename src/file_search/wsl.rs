//! WSL 上の TeX Live が返す Linux path と Windows UNC path の境界。
//!
//! distribution 一覧の表示形式は解釈せず、既定 distribution 内で公式 `wslpath -w /`
//! を実行して root と distribution 名を同時に得る。Linux の特殊なファイル名を
//! Windows 側の符号化へ推測変換せず、初期実装で可逆に扱える部分集合だけを受け入れる。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WslContext {
    distribution_name: OsString,
    unc_root: PathBuf,
}

impl WslContext {
    pub(super) fn distribution_name(&self) -> &OsStr {
        &self.distribution_name
    }

    pub(super) fn unc_root(&self) -> &Path {
        &self.unc_root
    }
}

pub(super) fn parse_wsl_root_output(bytes: &[u8]) -> Result<WslContext, &'static str> {
    let bytes = one_line(bytes)?;
    let value = std::str::from_utf8(bytes).map_err(|_| "WSL root is not valid UTF-8")?;
    let value = value.trim_end_matches(['\\', '/']);
    let without_leading = value.strip_prefix("\\\\").ok_or("WSL root is not UNC")?;
    let (server, distribution) = without_leading
        .split_once('\\')
        .ok_or("WSL root has no distribution share")?;
    if !(server.eq_ignore_ascii_case("wsl.localhost") || server.eq_ignore_ascii_case("wsl$")) {
        return Err("WSL root has an unexpected server");
    }
    if distribution.is_empty()
        || distribution == "."
        || distribution == ".."
        || distribution.contains(['\\', '/'])
        || distribution.chars().any(char::is_control)
    {
        return Err("WSL root has an invalid distribution share");
    }

    let mut canonical_root = String::from("\\\\");
    canonical_root.push_str(server);
    canonical_root.push('\\');
    canonical_root.push_str(distribution);
    canonical_root.push('\\');
    Ok(WslContext {
        distribution_name: OsString::from(distribution),
        unc_root: PathBuf::from(canonical_root),
    })
}

pub(super) fn linux_absolute_path_to_unc(
    context: &WslContext,
    linux_path: &str,
) -> Result<PathBuf, &'static str> {
    if !linux_path.starts_with('/') || linux_path.starts_with("//") {
        return Err("kpsewhich path is not an absolute Linux path");
    }
    let mut result = context.unc_root.as_os_str().to_os_string();
    let components = &linux_path[1..];
    if components.is_empty() {
        return Ok(PathBuf::from(result));
    }
    for (index, component) in components.split('/').enumerate() {
        validate_component(component)?;
        if index != 0 {
            result.push("\\");
        }
        result.push(component);
    }
    Ok(PathBuf::from(result))
}

pub(super) fn translate_linux_search_path(
    context: &WslContext,
    bytes: &[u8],
) -> Result<OsString, &'static str> {
    let bytes = one_line(bytes)?;
    let value = std::str::from_utf8(bytes).map_err(|_| "search path is not valid UTF-8")?;
    let mut translated = Vec::new();
    for raw_element in value.split(':') {
        if raw_element.is_empty() {
            return Err("search path contains an unexpanded default element");
        }
        let (database_only, element) = match raw_element.strip_prefix("!!") {
            Some(element) => (true, element),
            None => (false, raw_element),
        };
        if element == "." {
            translated.push(PathBuf::from("."));
            continue;
        }

        let recursive_run = find_recursive_run(element.as_bytes())?;
        let (prefix, suffix) = match recursive_run {
            Some((start, end)) => (&element[..start], Some(&element[end..])),
            None => (element, None),
        };
        let unc_prefix = linux_absolute_path_to_unc(context, prefix)?;
        let mut translated_element = OsString::new();
        if database_only {
            translated_element.push("!!");
        }
        translated_element.push(unc_prefix);
        if let Some(suffix) = suffix {
            translated_element.push("//");
            if !suffix.is_empty() {
                for component in suffix.split('/') {
                    validate_component(component)?;
                }
                translated_element.push(suffix);
            }
        }
        translated.push(PathBuf::from(translated_element));
    }
    std::env::join_paths(translated).map_err(|_| "translated search path cannot be joined")
}

fn one_line(mut bytes: &[u8]) -> Result<&[u8], &'static str> {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes = &bytes[..bytes.len() - 1];
    }
    if bytes.is_empty() {
        return Err("empty WSL output");
    }
    if bytes.contains(&0) || bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err("WSL output is not exactly one line");
    }
    Ok(bytes)
}

fn find_recursive_run(bytes: &[u8]) -> Result<Option<(usize, usize)>, &'static str> {
    let mut found = None;
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'/' && bytes[index + 1] == b'/' {
            let start = index;
            index += 2;
            while index < bytes.len() && bytes[index] == b'/' {
                index += 1;
            }
            if found.is_some() {
                return Err("search path contains multiple recursive markers");
            }
            found = Some((start, index));
        } else {
            index += 1;
        }
    }
    Ok(found)
}

fn validate_component(component: &str) -> Result<(), &'static str> {
    if component.is_empty() || component == "." || component == ".." {
        return Err("Linux path has an empty or relative component");
    }
    if component.ends_with(['.', ' '])
        || component.encode_utf16().count() > 255
        || component
            .chars()
            .any(|character| character <= '\u{1f}' || "<>:\"/\\|?*".contains(character))
    {
        return Err("Linux path component is not directly representable on Windows");
    }
    let stem = component.split('.').next().unwrap_or(component);
    let upper = stem.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            });
    if reserved {
        return Err("Linux path component is a reserved Windows name");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{linux_absolute_path_to_unc, parse_wsl_root_output, translate_linux_search_path};
    use std::path::PathBuf;

    #[test]
    fn wslpathのrootからdistributionとuncを同時に得る() {
        let context =
            parse_wsl_root_output("\\\\wsl.localhost\\Ubuntu 24.04 日本語\\\r\n".as_bytes())
                .unwrap();

        assert_eq!(context.distribution_name(), "Ubuntu 24.04 日本語");
        assert_eq!(
            context.unc_root(),
            PathBuf::from("\\\\wsl.localhost\\Ubuntu 24.04 日本語\\")
        );
    }

    #[test]
    fn 偽serverと余分な階層と複数行を拒む() {
        for output in [
            "\\\\example.invalid\\Ubuntu\\\n",
            "\\\\wsl.localhost\\Ubuntu\\extra\\\n",
            "\\\\wsl.localhost\\Ubuntu\\\nsecond\n",
        ] {
            assert!(parse_wsl_root_output(output.as_bytes()).is_err());
        }
    }

    #[test]
    fn linux絶対pathをuncへ写してunicodeと空白を保つ() {
        let context = parse_wsl_root_output(b"\\\\wsl.localhost\\Ubuntu\\\n").unwrap();
        let translated =
            linux_absolute_path_to_unc(&context, "/usr/local/share/日本 語/latex.ltx").unwrap();

        assert_eq!(
            translated,
            PathBuf::from("\\\\wsl.localhost\\Ubuntu\\usr\\local\\share\\日本 語\\latex.ltx")
        );
    }

    #[test]
    fn 相対pathとdotdotとwindows特殊名を推測変換しない() {
        let context = parse_wsl_root_output(b"\\\\wsl.localhost\\Ubuntu\\\n").unwrap();
        for path in ["relative/file", "/a/../b", "/a/name:stream", "/a/NUL"] {
            assert!(linux_absolute_path_to_unc(&context, path).is_err());
        }
    }

    #[test]
    fn linux探索pathを順序と再帰記号を保ってwindows形式へ写す() {
        let context = parse_wsl_root_output(b"\\\\wsl.localhost\\Ubuntu\\\n").unwrap();
        let translated = translate_linux_search_path(
            &context,
            b".:/home/user/texmf/tex///:!!/usr/local/texlive/texmf-dist/tex/latex//\n",
        )
        .unwrap();
        let elements = std::env::split_paths(&translated).collect::<Vec<_>>();

        assert_eq!(elements[0], PathBuf::from("."));
        assert_eq!(
            elements[1],
            PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\user\\texmf\\tex//")
        );
        assert_eq!(
            elements[2],
            PathBuf::from(
                "!!\\\\wsl.localhost\\Ubuntu\\usr\\local\\texlive\\texmf-dist\\tex\\latex//"
            )
        );
    }
}
