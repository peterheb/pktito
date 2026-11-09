//! pktito's versions as models.json's `needs` and core/pktito.h's
//! PKTITO_VERSION write them, major.minor.patch. build.rs compares the two
//! to leave out of the app what its engine can't load (catalog). It reads
//! this file by its path, and the app compiles it only for its tests, which
//! hold the comparison to the cases pktito's tests/editioncheck holds
//! core/editions.c's to.

/// A version's numbers, any missing ones 0 ("0.9" is 0.9.0); None for
/// what isn't a version. Versions compare as these arrays do, number by
/// number, so 0.9.10 is after 0.9.8.
pub fn version(v: &str) -> Option<[u64; 3]> {
    let mut parts = [0; 3];
    let mut n = 0;
    for p in v.split('.') {
        if n == parts.len() || p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        parts[n] = p.parse().ok()?;
        n += 1;
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::version;

    /// editioncheck's pairs: by number, not as text
    #[test]
    fn versions_compare_by_number() {
        let v = |s| version(s).unwrap();
        assert!(v("0.9.10") > v("0.9.8"));
        assert!(v("0.10.0") > v("0.9.6"));
        assert!(v("0.9.6") == v("0.9.6"));
        assert!(v("0.9.8") < v("0.9.10"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.9") == v("0.9.0"));
        assert!(v("99.0.0") > v("0.9.6"));
    }

    #[test]
    fn what_isnt_a_version() {
        for s in [
            "",
            "0.9.",
            ".9",
            "0.9.6.1",
            "0.9.x",
            "v0.9.6",
            "+1.0.0",
            "0.9.6-rc1",
        ] {
            assert_eq!(version(s), None, "{s:?}");
        }
    }
}
