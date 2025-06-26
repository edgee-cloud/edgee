use std::fmt;

use edgee_api_client::types as api_types;

#[derive(Clone)]
pub struct LanguageConfig {
    pub name: &'static str,
    pub repo_url: &'static str,
    pub default_build_command: &'static str,
    pub alias: &'static [&'static str],
    pub wit_world_extra: &'static str,
    pub deps_extra: &'static str,
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
    pub wit_world: &'static str,
    pub latest_wit_world_version: &'static str,
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
        wit_world_extra: "",
        deps_extra: "",
    },
    LanguageConfig {
        name: "CSharp",
        repo_url: "https://github.com/edgee-cloud/example-csharp-component",
        default_build_command: "dotnet build",
        alias: &["csharp", "cs", "c#"],
        wit_world_extra: "",
        deps_extra: "",
    },
    LanguageConfig {
        name: "Go",
        repo_url: "https://github.com/edgee-cloud/example-go-component",
        default_build_command: "go build -o main .",
        alias: &["go", "golang"],
        wit_world_extra: "\
  // for some reason, tinygo requires this include
  // cf https://component-model.bytecodealliance.org/language-support/go.html#2-determine-which-world-the-component-will-implement
  include wasi:cli/imports@0.2.0;
",
        deps_extra: "\
cli = \"https://github.com/WebAssembly/wasi-cli/archive/refs/tags/v0.2.0.tar.gz\"
",
    },
    LanguageConfig {
        name: "JavaScript",
        repo_url: "https://github.com/edgee-cloud/example-js-component",
        default_build_command: "node main.js",
        alias: &["js", "javascript"],
        wit_world_extra: "",
        deps_extra: "",
    },
    LanguageConfig {
        name: "Python",
        repo_url: "https://github.com/edgee-cloud/example-py-component",
        default_build_command: "python main.py",
        alias: &["py", "python"],
        wit_world_extra: "",
        deps_extra: "",
    },
    LanguageConfig {
        name: "Rust",
        repo_url: "https://github.com/edgee-cloud/example-rust-component",
        default_build_command: "cargo build --release",
        alias: &["rs", "rust"],
        wit_world_extra: "",
        deps_extra: "",
    },
    LanguageConfig {
        name: "TypeScript",
        repo_url: "https://github.com/edgee-cloud/example-ts-component",
        default_build_command: "npx tsc",
        alias: &["ts", "typescript"],
        wit_world_extra: "",
        deps_extra: "",
    },
];

pub static CATEGORY_OPTIONS: &[CategoryConfig] = &[
    CategoryConfig {
        name: "Data Collection",
        value: api_types::ComponentCreateInputCategory::DataCollection,
        wit_world: "data-collection",
        latest_wit_world_version: "1.0.1",
    },
    CategoryConfig {
        name: "Edge Function",
        value: api_types::ComponentCreateInputCategory::EdgeFunction,
        wit_world: "edge-function",
        latest_wit_world_version: "1.0.0",
    },
];

pub static DATA_COLLECTION_SUBCATEGORY_OPTIONS: &[SubCategoryConfig] = &[
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

pub static EDGE_FUNCTION_SUBCATEGORY_OPTIONS: &[SubCategoryConfig] = &[SubCategoryConfig {
    name: "Wasm Function",
    value: api_types::ComponentCreateInputSubcategory::WasmFunction,
}];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_config_display() {
        let config = LanguageConfig {
            name: "Rust",
            repo_url: "https://github.com/edgee-cloud/example-rust-component",
            default_build_command: "cargo build --release",
            alias: &["rs", "rust"],
            wit_world_extra: "",
            deps_extra: "",
        };
        assert_eq!(format!("{}", config), "Rust");
    }

    #[test]
    fn test_category_config_display() {
        let config = CategoryConfig {
            name: "Data Collection",
            value: api_types::ComponentCreateInputCategory::DataCollection,
            wit_world: "data-collection",
            latest_wit_world_version: "1.0.1",
        };
        assert_eq!(format!("{}", config), "Data Collection");
    }

    #[test]
    fn test_subcategory_config_display() {
        let config = SubCategoryConfig {
            name: "Analytics",
            value: api_types::ComponentCreateInputSubcategory::Analytics,
        };
        assert_eq!(format!("{}", config), "Analytics");
    }

    #[test]
    fn test_language_options() {
        let expected_languages = [
            "C",
            "CSharp",
            "Go",
            "JavaScript",
            "Python",
            "Rust",
            "TypeScript",
        ];
        for &expected in &expected_languages {
            assert!(LANGUAGE_OPTIONS
                .iter()
                .any(|config| config.name == expected));
        }
    }

    #[test]
    fn test_category_options() {
        assert_eq!(CATEGORY_OPTIONS.len(), 2); // for now!
        let expected_categories = ["Data Collection", "Edge Function"];
        for &expected in &expected_categories {
            assert!(CATEGORY_OPTIONS
                .iter()
                .any(|config| config.name == expected));
        }
    }

    #[test]
    fn test_subcategory_options() {
        let expected_subcategories = ["Analytics", "Attribution", "Warehouse", "Conversion API"];
        for &expected in &expected_subcategories {
            assert!(DATA_COLLECTION_SUBCATEGORY_OPTIONS
                .iter()
                .any(|config| config.name == expected));
        }
    }
}
