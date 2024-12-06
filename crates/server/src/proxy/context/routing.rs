use super::incoming::RequestHandle;
use crate::config;
use http::uri::PathAndQuery;
use regex::Regex;
use std::str::FromStr;

pub struct RoutingContext {
    pub backend: config::BackendConfiguration,
    pub path: PathAndQuery,
}

impl RoutingContext {
    pub fn from_request(request: &RequestHandle) -> Option<Self> {
        let cfg = &config::get().routing;
        let routing = cfg
            .iter()
            .find(|r| r.domain.as_str() == request.get_host().as_str())?
            .to_owned();
        let default_backend = routing.backends.iter().find(|b| b.default)?;

        let mut upstream_backend: Option<&config::BackendConfiguration> = None;
        let mut upstream_path: Option<PathAndQuery> = None;
        for rule in routing.rules {
            match (rule.path, rule.path_prefix, rule.path_regexp) {
                (Some(path), _, _) => {
                    if *request.get_path_and_query() == path {
                        upstream_backend = match rule.backend {
                            Some(name) => routing.backends.iter().find(|b| b.name == name),
                            None => Some(default_backend),
                        };
                        upstream_path = match rule.rewrite {
                            Some(replacement) => PathAndQuery::from_str(&replacement).ok(),
                            None => PathAndQuery::from_str(&path).ok(),
                        };
                        break;
                    }
                }
                (None, Some(prefix), _) => {
                    if request
                        .get_path_and_query()
                        .to_string()
                        .starts_with(&prefix)
                    {
                        upstream_backend = match rule.backend {
                            Some(name) => routing.backends.iter().find(|b| b.name == name),
                            None => Some(default_backend),
                        };
                        upstream_path = match rule.rewrite {
                            Some(replacement) => {
                                let new_path = request.get_path_and_query().to_string().replacen(
                                    &prefix,
                                    &replacement,
                                    1,
                                );
                                PathAndQuery::from_str(&new_path).ok()
                            }
                            None => Some(request.get_path_and_query().clone()),
                        };
                        break;
                    }
                }
                (None, None, Some(pattern)) => {
                    let regexp = Regex::new(&pattern).expect("regex pattern should be valid");
                    let path = request.get_path_and_query().to_string();
                    if regexp.is_match(&path) {
                        upstream_backend = match rule.backend {
                            Some(name) => routing.backends.iter().find(|b| b.name == name),
                            None => Some(default_backend),
                        };
                        upstream_path = match rule.rewrite {
                            Some(replacement) => {
                                PathAndQuery::from_str(&regexp.replacen(&path, 1, &replacement))
                                    .ok()
                            }
                            None => PathAndQuery::from_str(&path).ok(),
                        };
                        break;
                    }
                }
                (None, None, None) => {}
            }
        }

        let backend = upstream_backend.unwrap_or(default_backend).to_owned();
        let path = upstream_path.unwrap_or(request.get_path_and_query().clone());

        Some(Self { backend, path })
    }
}
