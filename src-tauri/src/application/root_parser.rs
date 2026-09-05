use std::error::Error;
use std::fmt;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootParseError {
    EmptyInput,
    InvalidUrl,
    UnsupportedHost,
    MissingFolderId,
    InvalidFolderId,
    RootContainerNotAllowed,
}

impl fmt::Display for RootParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "input cannot be empty"),
            Self::InvalidUrl => write!(f, "invalid Google Drive URL"),
            Self::UnsupportedHost => write!(f, "URL host must be drive.google.com"),
            Self::MissingFolderId => write!(f, "no folder ID found in URL"),
            Self::InvalidFolderId => {
                write!(
                    f,
                    "folder ID must be 15-64 alphanumeric characters, underscores, or dashes"
                )
            }
            Self::RootContainerNotAllowed => {
                write!(
                    f,
                    "'root' (My Drive container) cannot be selected as a migration root"
                )
            }
        }
    }
}

impl Error for RootParseError {}

fn is_valid_folder_id(id: &str) -> bool {
    if id.len() < 15 || id.len() > 64 {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn parse_root_input(input: &str) -> Result<String, RootParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(RootParseError::EmptyInput);
    }

    let folder_id = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let parsed_url = Url::parse(trimmed).map_err(|_| RootParseError::InvalidUrl)?;
        let host = parsed_url
            .host_str()
            .ok_or(RootParseError::UnsupportedHost)?;
        if host != "drive.google.com" {
            return Err(RootParseError::UnsupportedHost);
        }

        if let Some((_, id)) = parsed_url.query_pairs().find(|(k, _)| k == "id") {
            id.to_string()
        } else {
            let segments: Vec<&str> = parsed_url
                .path_segments()
                .map(|s| s.filter(|seg| !seg.is_empty()).collect())
                .unwrap_or_default();

            if let Some(pos) = segments.iter().position(|&seg| seg == "folders") {
                if let Some(&id) = segments.get(pos + 1) {
                    id.to_string()
                } else {
                    return Err(RootParseError::MissingFolderId);
                }
            } else {
                return Err(RootParseError::MissingFolderId);
            }
        }
    } else {
        trimmed.to_string()
    };

    if folder_id.eq_ignore_ascii_case("root") {
        return Err(RootParseError::RootContainerNotAllowed);
    }

    if !is_valid_folder_id(&folder_id) {
        return Err(RootParseError::InvalidFolderId);
    }

    Ok(folder_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_folder_id_and_trims_whitespace() {
        let input = "  1aBcDeFgHiJkLmNoPqRsTuVwXyZ_12345  \n";
        assert_eq!(
            parse_root_input(input),
            Ok("1aBcDeFgHiJkLmNoPqRsTuVwXyZ_12345".to_string())
        );
    }

    #[test]
    fn parses_standard_drive_folder_url() {
        let url = "https://drive.google.com/drive/folders/1aBcDeFgHiJkLmNoPqRsTuVwXyZ";
        assert_eq!(
            parse_root_input(url),
            Ok("1aBcDeFgHiJkLmNoPqRsTuVwXyZ".to_string())
        );
    }

    #[test]
    fn parses_multi_account_drive_folder_url_with_query_and_fragment() {
        let url = "https://drive.google.com/drive/u/2/folders/1aBcDeFgHiJkLmNoPqRsTuVwXyZ?usp=sharing#frag";
        assert_eq!(
            parse_root_input(url),
            Ok("1aBcDeFgHiJkLmNoPqRsTuVwXyZ".to_string())
        );
    }

    #[test]
    fn parses_legacy_open_url_with_id_param() {
        let url = "https://drive.google.com/open?id=1aBcDeFgHiJkLmNoPqRsTuVwXyZ&authuser=0";
        assert_eq!(
            parse_root_input(url),
            Ok("1aBcDeFgHiJkLmNoPqRsTuVwXyZ".to_string())
        );
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse_root_input("   "), Err(RootParseError::EmptyInput));
    }

    #[test]
    fn rejects_non_drive_host() {
        let url = "https://dropbox.com/s/1aBcDeFgHiJkLmNoPqRsTuVwXyZ";
        assert_eq!(parse_root_input(url), Err(RootParseError::UnsupportedHost));
    }

    #[test]
    fn rejects_root_container_id() {
        assert_eq!(
            parse_root_input("root"),
            Err(RootParseError::RootContainerNotAllowed)
        );
        assert_eq!(
            parse_root_input("https://drive.google.com/drive/folders/root"),
            Err(RootParseError::RootContainerNotAllowed)
        );
    }

    #[test]
    fn rejects_invalid_id_characters_and_length() {
        assert_eq!(
            parse_root_input("too_short_id"),
            Err(RootParseError::InvalidFolderId)
        );
        assert_eq!(
            parse_root_input("invalid!characters*inside*folder*id"),
            Err(RootParseError::InvalidFolderId)
        );
    }
}
