use std::collections::HashMap;

pub fn check_cacheability(
    headers: &HashMap<String, String>,
    has_shared_cache_before: bool,
) -> bool {
    if !has_shared_cache_before {
        is_cacheable_by_browser(headers)
    } else {
        is_cacheable_by_shared_cache(headers) || is_cacheable_by_browser(headers)
    }
}

fn is_cacheable_by_browser(headers: &HashMap<String, String>) -> bool {
    let cache_control = headers
        .get("cache-control")
        .map_or("".to_string(), |v| v.to_lowercase());
    let has_no_cache = cache_control.contains("no-cache");
    let must_revalidate = cache_control.contains("must-revalidate");
    let expires = headers.get("expires");
    let etag = headers.get("etag");
    let last_modified = headers.get("last-modified");

    // Cache-Control: no-store means no caching at all
    if cache_control.contains("no-store") {
        return false;
    }

    // no-cache means the browser must revalidate with the server... so it's cacheable
    if has_no_cache {
        return true;
    }

    // must-revalidate means the browser must revalidate with the server... so it's cacheable
    if must_revalidate {
        return true;
    }

    // ETag and Last-Modified headers are used for cache validation
    if etag.is_some() || last_modified.is_some() {
        return true;
    }

    if expires.is_some() {
        return true;
    }

    // max-age is used when Expires header is missing
    let max_age = extract_max_age(&cache_control, "max-age");
    if let Some(age) = max_age {
        if age > 0 {
            return true;
        }
    }

    false
}

fn is_cacheable_by_shared_cache(headers: &HashMap<String, String>) -> bool {
    let cache_control = headers
        .get("cache-control")
        .map_or("".to_string(), |v| v.to_lowercase());
    let surrogate_control = headers
        .get("surrogate-control")
        .map_or("".to_string(), |v| v.to_lowercase());
    let is_private = cache_control.contains("private");
    let s_max_age = extract_max_age(&cache_control, "s-maxage");
    let max_age = extract_max_age(&cache_control, "max-age");
    let surrogate_max_age = extract_max_age(&surrogate_control, "max-age");
    let surrogate_s_max_age = extract_max_age(&surrogate_control, "s-maxage");

    // if surrogate control exists, we prefer it over cache control
    if !surrogate_control.is_empty() {
        // Check for Surrogate-Control: no-store
        if surrogate_control.contains("no-store") {
            return false;
        }

        if surrogate_max_age.unwrap_or(0) > 0 {
            return true;
        }

        if surrogate_s_max_age.unwrap_or(0) > 0 {
            return true;
        }
    } else {
        // Check for Cache-Control: no-store
        if cache_control.contains("no-store") {
            return false;
        }

        if !is_private && max_age.unwrap_or(0) > 0 {
            return true;
        }
        if s_max_age.unwrap_or(0) > 0 {
            return true;
        }
    }

    false
}

fn extract_max_age(cache_control: &str, directive: &str) -> Option<i64> {
    let pattern = format!("{directive}=");
    cache_control.split(',').find_map(|part| {
        let trimmed = part.trim();
        if trimmed.starts_with(&pattern) {
            trimmed[pattern.len()..].parse().ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_headers(pairs: Vec<(&str, &str)>) -> HashMap<String, String> {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn check_cacheability_with_no_shared_cache_and_cacheable_by_browser() {
        let headers = create_headers(vec![("cache-control", "max-age=3600")]);
        assert!(check_cacheability(&headers, false));
    }

    #[test]
    fn check_cacheability_with_no_shared_cache_and_not_cacheable_by_browser() {
        let headers = create_headers(vec![("cache-control", "no-store")]);
        assert!(!check_cacheability(&headers, false));
    }

    #[test]
    fn check_cacheability_with_shared_cache_and_cacheable_by_both() {
        let headers = create_headers(vec![
            ("cache-control", "max-age=3600"),
            ("surrogate-control", "max-age=3600"),
        ]);
        assert!(check_cacheability(&headers, true));
    }

    #[test]
    fn check_cacheability_with_shared_cache_and_not_cacheable_by_shared_cache() {
        let headers = create_headers(vec![
            ("cache-control", "private"),
            ("surrogate-control", "no-store"),
        ]);
        assert!(!check_cacheability(&headers, true));
    }

    #[test]
    fn check_cacheability_with_shared_cache_and_not_cacheable_by_browser() {
        let headers = create_headers(vec![
            ("cache-control", "no-store"),
            ("surrogate-control", "max-age=3600"),
        ]);
        assert!(check_cacheability(&headers, true));
    }

    #[test]
    fn is_cacheable_by_browser_with_no_store() {
        let headers = create_headers(vec![("cache-control", "no-store")]);
        assert!(!is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_no_cache() {
        let headers = create_headers(vec![("cache-control", "no-cache")]);
        assert!(is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_must_revalidate() {
        let headers = create_headers(vec![("cache-control", "must-revalidate")]);
        assert!(is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_etag() {
        let headers = create_headers(vec![("etag", "some-etag")]);
        assert!(is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_expires() {
        let headers = create_headers(vec![("expires", "some-date")]);
        assert!(is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_max_age() {
        let headers = create_headers(vec![("cache-control", "max-age=3600")]);
        assert!(is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_browser_with_max_age_zero() {
        let headers = create_headers(vec![("cache-control", "max-age=0")]);
        assert!(!is_cacheable_by_browser(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_no_store() {
        let headers = create_headers(vec![("cache-control", "no-store")]);
        assert!(!is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_surrogate_no_store() {
        let headers = create_headers(vec![("surrogate-control", "no-store")]);
        assert!(!is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_surrogate_max_age() {
        let headers = create_headers(vec![("surrogate-control", "max-age=3600")]);
        assert!(is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_surrogate_max_age_zero() {
        let headers = create_headers(vec![("surrogate-control", "max-age=0")]);
        assert!(!is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_surrogate_s_max_age() {
        let headers = create_headers(vec![("surrogate-control", "s-maxage=3600")]);
        assert!(is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_max_age_and_not_private() {
        let headers = create_headers(vec![("cache-control", "max-age=3600")]);
        assert!(is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_s_max_age() {
        let headers = create_headers(vec![("cache-control", "s-maxage=3600")]);
        assert!(is_cacheable_by_shared_cache(&headers));
    }

    #[test]
    fn is_cacheable_by_shared_cache_with_max_age_and_private() {
        let headers = create_headers(vec![("cache-control", "private, max-age=3600")]);
        assert!(!is_cacheable_by_shared_cache(&headers));
    }
}
