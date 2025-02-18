#[derive(Clone)]
pub struct LanguageConfig {
    pub name: &'static str,
    pub repo_url: &'static str,
    pub default_build_command: &'static str,
    pub alias: &'static [&'static str],
}

impl std::fmt::Display for LanguageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct CategoryConfig {
    pub name: &'static str,
    pub value: &'static edgee_api_client::types::ComponentCreateInputCategory,
}

impl std::fmt::Display for CategoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct SubCategoryConfig {
    pub name: &'static str,
    pub value: &'static edgee_api_client::types::ComponentCreateInputSubcategory,
}

impl std::fmt::Display for SubCategoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub static LANGUAGE_OPTIONS: &[LanguageConfig] = &[
    LanguageConfig {
        name: "C",
        repo_url: "https://github.com/edgee-cloud/example-c-component",
        default_build_command: "gcc main.c -o main",
        alias: &["c"],
    },
    LanguageConfig {
        name: "CSharp",
        repo_url: "https://github.com/edgee-cloud/example-csharp-component",
        default_build_command: "dotnet build",
        alias: &["csharp", "cs", "c#"],
    },
    LanguageConfig {
        name: "Go",
        repo_url: "https://github.com/edgee-cloud/example-go-component",
        default_build_command: "go build -o main .",
        alias: &["go", "golang"],
    },
    LanguageConfig {
        name: "JavaScript",
        repo_url: "https://github.com/edgee-cloud/example-js-component",
        default_build_command: "node main.js",
        alias: &["js", "javascript"],
    },
    LanguageConfig {
        name: "Python",
        repo_url: "https://github.com/edgee-cloud/example-py-component",
        default_build_command: "python main.py",
        alias: &["py", "python"],
    },
    LanguageConfig {
        name: "Rust",
        repo_url: "https://github.com/edgee-cloud/example-rust-component",
        default_build_command: "cargo build --release",
        alias: &["rs", "rust"],
    },
    LanguageConfig {
        name: "TypeScript",
        repo_url: "https://github.com/edgee-cloud/example-ts-component",
        default_build_command: "npx tsc",
        alias: &["ts", "typescript"],
    },
];

pub static CATEGORY_OPTIONS: &[CategoryConfig] = &[CategoryConfig {
    name: "Data Collection",
    value: &edgee_api_client::types::ComponentCreateInputCategory::DataCollection,
}];

pub static SUBCATEGORY_OPTIONS: &[SubCategoryConfig] = &[
    SubCategoryConfig {
        name: "Analytics",
        value: &edgee_api_client::types::ComponentCreateInputSubcategory::Analytics,
    },
    SubCategoryConfig {
        name: "Attribution",
        value: &edgee_api_client::types::ComponentCreateInputSubcategory::Attribution,
    },
    SubCategoryConfig {
        name: "Warehouse",
        value: &edgee_api_client::types::ComponentCreateInputSubcategory::Warehouse,
    },
];
