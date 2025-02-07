#[derive(Clone)]
pub struct LanguageConfig {
    pub name: &'static str,
    pub repo_url: &'static str,
}

impl std::fmt::Display for LanguageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub const LANGUAGE_OPTIONS: &'static [LanguageConfig] = &[
    LanguageConfig {
        name: "Rust",
        repo_url: "https://github.com/edgee-cloud/example-rust-component",
    },
    LanguageConfig {
        name: "Go",
        repo_url: "https://github.com/edgee-cloud/example-go-component",
    },
    LanguageConfig {
        name: "Python",
        repo_url: "https://github.com/edgee-cloud/example-py-component",
    },
    LanguageConfig {
        name: "JavaScript",
        repo_url: "https://github.com/edgee-cloud/example-js-component",
    },
    LanguageConfig {
        name: "CSharp",
        repo_url: "https://github.com/edgee-cloud/example-csharp-component",
    },
    LanguageConfig {
        name: "C",
        repo_url: "https://github.com/edgee-cloud/example-c-component",
    },
];
