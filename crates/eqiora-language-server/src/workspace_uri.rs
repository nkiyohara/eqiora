use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use lsp_types::Uri;

pub(crate) fn file_uri_path(uri: &Uri) -> Option<PathBuf> {
    if !uri.scheme()?.as_str().eq_ignore_ascii_case("file")
        || uri.authority().is_some_and(|authority| {
            !authority.as_str().is_empty() && !authority.as_str().eq_ignore_ascii_case("localhost")
        })
    {
        return None;
    }
    let path = uri
        .path()
        .as_estr()
        .decode()
        .into_string()
        .ok()?
        .into_owned();
    #[cfg(windows)]
    let path = if path.starts_with('/') && path.as_bytes().get(2) == Some(&b':') {
        path[1..].to_owned()
    } else {
        path
    };
    Some(PathBuf::from(path))
}

pub(crate) fn project_file_uri(root: &str, relative: &Path) -> Option<Uri> {
    let path = relative.to_str()?.replace(std::path::MAIN_SEPARATOR, "/");
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    Uri::from_str(&format!("{root}{encoded}")).ok()
}
