//! Read-only Steam discovery; filesystem roots are injectable in tests.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{Result, invalid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub root: PathBuf,
    pub cfg: PathBuf,
    pub log: PathBuf,
}

impl Installation {
    pub fn validate(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let cfg = root.join("game/csgo/cfg");
        if !root.join("game/bin/win64/cs2.exe").is_file() || !cfg.is_dir() {
            return Err(invalid("CS2 executable or game/csgo/cfg directory was not found").into());
        }
        Ok(Self {
            log: root.join("game/csgo/console.log"),
            root,
            cfg,
        })
    }
}

pub fn discover(steam_roots: &[PathBuf]) -> Result<Installation> {
    let mut libraries = Vec::new();
    for root in steam_roots {
        libraries.push(root.clone());
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))
            && let Ok(entries) = parse_vdf(&text)
            && let Some(Value::Object(folders)) = get(&entries, "libraryfolders")
        {
            for (index, value) in folders {
                if !index.bytes().all(|b| b.is_ascii_digit()) || index.is_empty() {
                    continue;
                }
                let path = match value {
                    Value::Text(path) => Some(path.as_str()),
                    Value::Object(fields) => text_value(fields, "path"),
                };
                if let Some(path) = path {
                    libraries.push(PathBuf::from(path));
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for library in libraries {
        if !seen.insert(library.clone()) {
            continue;
        }
        let Ok(text) = fs::read_to_string(library.join("steamapps/appmanifest_730.acf")) else {
            continue;
        };
        let Ok(entries) = parse_vdf(&text) else {
            continue;
        };
        let Some(Value::Object(app)) = get(&entries, "AppState") else {
            continue;
        };
        if text_value(app, "appid") != Some("730") {
            continue;
        }
        let Some(directory) = text_value(app, "installdir") else {
            continue;
        };
        let mut components = Path::new(directory).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || directory.contains([':', '/', '\\'])
        {
            continue;
        }
        if let Ok(installation) =
            Installation::validate(library.join("steamapps/common").join(directory))
        {
            return Ok(installation);
        }
    }
    Err(invalid(
        "CS2 was not found in Steam libraries. Install CS2 through Steam, then restart Utilizer.",
    )
    .into())
}

pub fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        use winreg::{
            RegKey,
            enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
        };
        for (hive, key, value) in [
            (HKEY_CURRENT_USER, "Software\\Valve\\Steam", "SteamPath"),
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\WOW6432Node\\Valve\\Steam",
                "InstallPath",
            ),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\Valve\\Steam", "InstallPath"),
        ] {
            if let Ok(key) = RegKey::predef(hive).open_subkey(key)
                && let Ok(path) = key.get_value::<String, _>(value)
            {
                roots.push(PathBuf::from(path));
            }
        }
    }
    for variable in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(path) = std::env::var_os(variable) {
            roots.push(PathBuf::from(path).join("Steam"));
        }
    }
    roots
}

#[derive(Debug)]
enum Value {
    Text(String),
    Object(Vec<(String, Value)>),
}

fn get<'a>(fields: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
}

fn text_value<'a>(fields: &'a [(String, Value)], key: &str) -> Option<&'a str> {
    match get(fields, key)? {
        Value::Text(text) => Some(text),
        Value::Object(_) => None,
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    Open,
    Close,
}

// Steam's small KeyValues metadata format: quoted/unquoted tokens, braces,
// escaped quotes/backslashes and line comments. Never regex Windows paths.
fn parse_vdf(text: &str) -> Result<Vec<(String, Value)>> {
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                        value.push(chars.next().expect("peeked escape"));
                    } else {
                        value.push(c);
                    }
                }
                if !closed {
                    return Err(invalid("Unterminated Steam metadata string").into());
                }
                tokens.push(Token::Text(value));
            }
            _ => {
                let mut value = String::from(c);
                while let Some(c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}') {
                        break;
                    }
                    value.push(chars.next().expect("peeked token"));
                }
                tokens.push(Token::Text(value));
            }
        }
    }
    fn object(
        tokens: &mut std::vec::IntoIter<Token>,
        nested: bool,
        depth: usize,
    ) -> Result<Vec<(String, Value)>> {
        if depth > 16 {
            return Err(invalid("Steam metadata nesting is too deep").into());
        }
        let mut fields = Vec::new();
        while let Some(token) = tokens.next() {
            match token {
                Token::Close if nested => return Ok(fields),
                Token::Text(key) => {
                    let value = match tokens.next() {
                        Some(Token::Text(text)) => Value::Text(text),
                        Some(Token::Open) => Value::Object(object(tokens, true, depth + 1)?),
                        _ => return Err(invalid("Invalid Steam metadata value").into()),
                    };
                    fields.push((key, value));
                }
                _ => return Err(invalid("Invalid Steam metadata key").into()),
            }
        }
        if nested {
            return Err(invalid("Unclosed Steam metadata object").into());
        }
        Ok(fields)
    }
    object(&mut tokens.into_iter(), false, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyvalues_paths_comments_and_malformed_inputs() {
        let fields = parse_vdf(
            r#"// comment
            "LibraryFolders" { "0" { "path" "D:\\Steam Library" } }
        "#,
        )
        .unwrap();
        let Some(Value::Object(libraries)) = get(&fields, "libraryfolders") else {
            panic!()
        };
        let Some(Value::Object(library)) = get(libraries, "0") else {
            panic!()
        };
        assert_eq!(text_value(library, "path"), Some(r"D:\Steam Library"));
        for text in ["\"unfinished", "key {", "key }", "}"] {
            assert!(parse_vdf(text).is_err(), "{text}");
        }
    }
}
