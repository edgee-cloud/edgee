use std::fmt;

use edgee_api_client::types as api_types;

#[derive(Clone)]
pub struct LanguageConfig {
    pub name: &'static str,
    pub repo_url: &'static str,
    pub default_build_command: &'static str,
    pub alias: &'static [&'static str],
}

impl fmt::Display for LanguageConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct CategoryConfig {
    pub name: &'static str,
    pub value: api_types::ComponentCreateInputCategory,
    pub wit_world: &'static [u8],
}

impl fmt::Display for CategoryConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct SubCategoryConfig {
    pub name: &'static str,
    pub value: api_types::ComponentCreateInputSubcategory,
}

impl fmt::Display for SubCategoryConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    value: api_types::ComponentCreateInputCategory::DataCollection,
    wit_world: include_bytes!("wit-world/data-collection.wit"),
}];

pub static SUBCATEGORY_OPTIONS: &[SubCategoryConfig] = &[
    SubCategoryConfig {
        name: "Analytics",
        value: api_types::ComponentCreateInputSubcategory::Analytics,
    },
    SubCategoryConfig {
        name: "Attribution",
        value: api_types::ComponentCreateInputSubcategory::Attribution,
    },
    SubCategoryConfig {
        name: "Warehouse",
        value: api_types::ComponentCreateInputSubcategory::Warehouse,
    },
    SubCategoryConfig {
        name: "Conversion API",
        value: api_types::ComponentCreateInputSubcategory::ConversionApi,
    },
];
