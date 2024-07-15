use std::str::FromStr;

use http::uri::PathAndQuery;
use regex::Regex;

use crate::config::{self, BackendConfiguration, RoutingRulesConfiguration};

use super::incoming_context::IncomingContext;

pub struct RoutingContext {
    pub backend: BackendConfiguration,
    pub path: PathAndQuery,
    pub rule: RoutingRulesConfiguration,
}

impl RoutingContext {
    pub fn from_request_context(ctx: &IncomingContext) -> Option<Self> {
        let cfg = &config::get().routing;
        let routing = cfg.iter().find(|r| r.domain == *ctx.host())?.to_owned();
        let default_backend = routing.backends.iter().find(|b| b.default)?;

        let mut upstream_backend: Option<&config::BackendConfiguration> = None;
        let mut upstream_path: Option<PathAndQuery> = None;
        let mut current_rule = RoutingRulesConfiguration::default();
        for rule in routing.rules {
            current_rule = rule.clone();
            match (rule.path, rule.path_prefix, rule.path_regexp) {
                (Some(path), _, _) => {
                    if *ctx.path() == path {
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
                    if ctx.path().to_string().starts_with(&prefix) {
                        upstream_backend = match rule.backend {
                            Some(name) => routing.backends.iter().find(|b| b.name == name),
                            None => Some(default_backend),
                        };
                        upstream_path = match rule.rewrite {
                            Some(replacement) => {
                                let new_path =
                                    ctx.path().to_string().replacen(&prefix, &replacement, 1);
                                PathAndQuery::from_str(&new_path).ok()
                            }
                            None => Some(ctx.path().clone()),
                        };
                        break;
                    }
                }
                (None, None, Some(pattern)) => {
                    let regexp = Regex::new(&pattern).expect("regex pattern should be valid");
                    let path = ctx.path().to_string();
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
        let path = upstream_path.unwrap_or(ctx.path().clone());

        Some(Self {
            backend,
            path,
            rule: current_rule,
        })
    }
}
