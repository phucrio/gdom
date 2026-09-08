use semver::Version;
use url::Url;

pub(super) fn is_newer_stable(current: &Version, offered: &Version) -> bool {
    offered.pre.is_empty() && offered > current
}

pub(super) fn is_release_artifact(url: &Url, version: &str) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url
            .path()
            .starts_with(&format!("/phucrio/gdom/releases/download/v{version}/"))
        && url
            .path_segments()
            .is_some_and(|segments| segments.count() == 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_newer_stable_versions_are_offered() {
        let installed = Version::parse("0.1.0").unwrap();
        for (version, expected) in [
            ("0.1.1", true),
            ("0.2.0", true),
            ("0.1.0", false),
            ("0.0.9", false),
            ("0.2.0-rc.1", false),
        ] {
            assert_eq!(
                is_newer_stable(&installed, &Version::parse(version).unwrap()),
                expected
            );
        }
    }

    #[test]
    fn release_artifacts_must_belong_to_this_repository_and_version() {
        for (address, expected) in [
            (
                "https://github.com/phucrio/gdom/releases/download/v0.1.1/app.exe",
                true,
            ),
            (
                "http://github.com/phucrio/gdom/releases/download/v0.1.1/app.exe",
                false,
            ),
            (
                "https://github.com/other/gdom/releases/download/v0.1.1/app.exe",
                false,
            ),
            (
                "https://github.com/phucrio/gdom/releases/download/v0.2.0/app.exe",
                false,
            ),
            (
                "https://github.com@evil.example/phucrio/gdom/releases/download/v0.1.1/app.exe",
                false,
            ),
            (
                "https://github.com/phucrio/gdom/releases/download/v0.1.1/app.exe?token=secret",
                false,
            ),
        ] {
            assert_eq!(
                is_release_artifact(&Url::parse(address).unwrap(), "0.1.1"),
                expected
            );
        }
    }
}
