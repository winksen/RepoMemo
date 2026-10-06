//! What kind of project a repository is, read from its manifests and layout.
//!
//! Deterministic: package manifests (`package.json`, `composer.json`,
//! `Cargo.toml`, ...) and well-known files (`artisan`, `manage.py`, ...) are
//! matched against a table of frameworks. No AI and no network are involved,
//! so the result is the same on every sync of the same commit.

use std::collections::{BTreeMap, BTreeSet};

use repomemo_domain::{RepoFile, RepoStack, RepoTechnology};
use serde_json::Value;

/// Manifest file names read for detection.
pub(crate) const STACK_MANIFESTS: &[&str] = &[
    "package.json", "composer.json", "Cargo.toml", "requirements.txt", "pyproject.toml", "Pipfile",
    "go.mod", "pom.xml", "build.gradle", "build.gradle.kts", "Gemfile",
];
/// Manifests larger than this are skipped, and no more than
/// `MAX_STACK_MANIFESTS` are read, so a huge monorepo stays cheap to sync.
pub(crate) const MAX_MANIFEST_BYTES: i64 = 512 * 1024;
pub(crate) const MAX_STACK_MANIFESTS: usize = 24;

/// Languages that say little about what a project is.
const NON_PRIMARY_LANGUAGES: &[&str] = &[
    "Markdown", "JSON", "YAML", "TOML", "Text", "Other", "CSS", "SCSS", "HTML", "XML", "SQL", "Shell",
];

struct Rule {
    /// Dependency or crate name looked up in manifests.
    dependency: &'static str,
    name: &'static str,
    category: &'static str,
}

const fn rule(dependency: &'static str, name: &'static str, category: &'static str) -> Rule {
    Rule { dependency, name, category }
}

/// `package.json` dependencies, in the order they are reported.
const NODE_RULES: &[Rule] = &[
    rule("next", "Next.js", "meta-framework"),
    rule("nuxt", "Nuxt", "meta-framework"),
    rule("@sveltejs/kit", "SvelteKit", "meta-framework"),
    rule("astro", "Astro", "meta-framework"),
    rule("@remix-run/react", "Remix", "meta-framework"),
    rule("react-native", "React Native", "mobile"),
    rule("expo", "Expo", "mobile"),
    rule("react", "React", "frontend"),
    rule("vue", "Vue", "frontend"),
    rule("@angular/core", "Angular", "frontend"),
    rule("svelte", "Svelte", "frontend"),
    rule("solid-js", "Solid", "frontend"),
    rule("electron", "Electron", "desktop"),
    rule("@tauri-apps/api", "Tauri", "desktop"),
    rule("@nestjs/core", "NestJS", "backend"),
    rule("express", "Express", "backend"),
    rule("fastify", "Fastify", "backend"),
    rule("koa", "Koa", "backend"),
    rule("hono", "Hono", "backend"),
    rule("vite", "Vite", "tooling"),
    rule("webpack", "Webpack", "tooling"),
    rule("tailwindcss", "Tailwind CSS", "tooling"),
    rule("typescript", "TypeScript", "language"),
    rule("prisma", "Prisma", "database"),
    rule("@prisma/client", "Prisma", "database"),
    rule("mongoose", "MongoDB", "database"),
    rule("jest", "Jest", "testing"),
    rule("vitest", "Vitest", "testing"),
    rule("@playwright/test", "Playwright", "testing"),
    rule("cypress", "Cypress", "testing"),
];

const COMPOSER_RULES: &[Rule] = &[
    rule("laravel/framework", "Laravel", "backend"),
    rule("symfony/framework-bundle", "Symfony", "backend"),
    rule("slim/slim", "Slim", "backend"),
    rule("livewire/livewire", "Livewire", "frontend"),
    rule("inertiajs/inertia-laravel", "Inertia", "frontend"),
    rule("phpunit/phpunit", "PHPUnit", "testing"),
];

const PYTHON_RULES: &[Rule] = &[
    rule("django", "Django", "backend"),
    rule("flask", "Flask", "backend"),
    rule("fastapi", "FastAPI", "backend"),
    rule("pytest", "pytest", "testing"),
    rule("sqlalchemy", "SQLAlchemy", "database"),
    rule("pandas", "pandas", "data"),
    rule("torch", "PyTorch", "data"),
    rule("tensorflow", "TensorFlow", "data"),
];

const RUST_RULES: &[Rule] = &[
    rule("tauri", "Tauri", "desktop"),
    rule("axum", "Axum", "backend"),
    rule("actix-web", "Actix Web", "backend"),
    rule("rocket", "Rocket", "backend"),
    rule("leptos", "Leptos", "frontend"),
    rule("yew", "Yew", "frontend"),
    rule("bevy", "Bevy", "game"),
    rule("clap", "clap", "cli"),
    rule("sqlx", "SQLx", "database"),
    rule("tokio", "Tokio", "runtime"),
];

const GO_RULES: &[Rule] = &[
    rule("gin-gonic/gin", "Gin", "backend"),
    rule("labstack/echo", "Echo", "backend"),
    rule("gofiber/fiber", "Fiber", "backend"),
    rule("spf13/cobra", "Cobra", "cli"),
];

const RUBY_RULES: &[Rule] = &[
    rule("rails", "Rails", "backend"),
    rule("sinatra", "Sinatra", "backend"),
    rule("rspec", "RSpec", "testing"),
];

const JVM_RULES: &[Rule] = &[
    rule("spring-boot", "Spring Boot", "backend"),
    rule("quarkus", "Quarkus", "backend"),
    rule("ktor", "Ktor", "backend"),
];

/// The language a backend framework is written in.
fn backend_language(name: &str) -> &'static str {
    match name {
        "Laravel" | "Symfony" | "Slim" => "PHP",
        "Django" | "Flask" | "FastAPI" => "Python",
        "Axum" | "Actix Web" | "Rocket" => "Rust",
        "Gin" | "Echo" | "Fiber" => "Go",
        "Rails" | "Sinatra" => "Ruby",
        "Spring Boot" | "Quarkus" | "Ktor" => "JVM",
        _ => "Node.js",
    }
}

/// Whether `name` appears in `text` as a whole token, so `axum` matches
/// `axum = "0.8"` but not `axum-extra` or `flaxum`.
fn mentions(text: &str, name: &str) -> bool {
    let token = |character: char| character.is_ascii_alphanumeric() || character == '-' || character == '_';
    text.match_indices(name).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + name.len()..].chars().next();
        !before.is_some_and(token) && !after.is_some_and(token)
    })
}

fn node_dependencies(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(Value::Object(manifest)) = serde_json::from_str::<Value>(text) else {
        return names;
    };
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(Value::Object(entries)) = manifest.get(section) {
            names.extend(entries.keys().cloned());
        }
    }
    names
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Detects the project's stack. `manifests` maps repository paths to the text
/// of the manifests that were read. Returns `None` when nothing identifies it.
pub(crate) fn detect_stack(files: &[RepoFile], manifests: &BTreeMap<String, String>) -> Option<RepoStack> {
    let mut found: Vec<RepoTechnology> = Vec::new();
    let mut add = |name: &str, category: &str| {
        if !found.iter().any(|technology| technology.name == name) {
            found.push(RepoTechnology { name: name.to_owned(), category: category.to_owned() });
        }
    };

    let mut node = BTreeSet::new();
    let (mut composer, mut python, mut rust, mut go, mut ruby, mut jvm) =
        (String::new(), String::new(), String::new(), String::new(), String::new(), String::new());
    for (path, text) in manifests {
        match file_name(path) {
            "package.json" => node.extend(node_dependencies(text)),
            "composer.json" => composer.push_str(&text.to_lowercase()),
            "requirements.txt" | "pyproject.toml" | "Pipfile" => python.push_str(&text.to_lowercase()),
            "Cargo.toml" => rust.push_str(&text.to_lowercase()),
            "go.mod" => go.push_str(&text.to_lowercase()),
            "Gemfile" => ruby.push_str(&text.to_lowercase()),
            _ => jvm.push_str(&text.to_lowercase()),
        }
    }
    for entry in NODE_RULES {
        if node.contains(entry.dependency) {
            add(entry.name, entry.category);
        }
    }
    let tables: [(&[Rule], &String); 6] = [
        (COMPOSER_RULES, &composer),
        (PYTHON_RULES, &python),
        (RUST_RULES, &rust),
        (GO_RULES, &go),
        (RUBY_RULES, &ruby),
        (JVM_RULES, &jvm),
    ];
    for (rules, text) in tables {
        for entry in rules {
            if mentions(text, entry.dependency) {
                add(entry.name, entry.category);
            }
        }
    }

    // Well-known files that identify a framework without a manifest entry.
    let has_path = |predicate: &dyn Fn(&str) -> bool| files.iter().any(|file| predicate(&file.path));
    if has_path(&|path| path == "artisan") && has_path(&|path| path.starts_with("app/Http/")) {
        add("Laravel", "backend");
    }
    if has_path(&|path| file_name(path) == "manage.py") {
        add("Django", "backend");
    }
    if has_path(&|path| file_name(path) == "angular.json") {
        add("Angular", "frontend");
    }
    if has_path(&|path| file_name(path) == "tauri.conf.json") {
        add("Tauri", "desktop");
    }
    if has_path(&|path| file_name(path) == "Dockerfile" || file_name(path).starts_with("docker-compose")) {
        add("Docker", "tooling");
    }
    if has_path(&|path| path.starts_with(".github/workflows/")) {
        add("GitHub Actions", "tooling");
    }

    // The language that holds the most code.
    let mut bytes: BTreeMap<&str, i64> = BTreeMap::new();
    for file in files {
        if let Some(language) = file.language.as_deref().filter(|language| !NON_PRIMARY_LANGUAGES.contains(language)) {
            *bytes.entry(language).or_default() += file.size_bytes;
        }
    }
    let primary = bytes.iter().max_by_key(|(_, size)| **size).map(|(language, _)| (*language).to_owned());
    if let Some(language) = &primary {
        add(language, "language");
    }

    let first = |category: &str| {
        found.iter().find(|technology| technology.category == category).map(|technology| technology.name.clone())
    };
    let frontend = first("frontend");
    let meta = first("meta-framework");
    let backend = first("backend");
    let desktop = first("desktop");
    let mobile = first("mobile");
    let is_cli = first("cli").is_some();
    let typescript = found.iter().any(|technology| technology.name == "TypeScript");

    let (kind, summary) = if let Some(desktop) = &desktop {
        let interface = frontend.as_ref().map(|name| format!(" with a {name} interface")).unwrap_or_default();
        let server = backend
            .as_ref()
            .map(|name| format!(" and a {name} {} backend", backend_language(name)))
            .unwrap_or_default();
        ("desktop_app", format!("{desktop} desktop app{interface}{server}"))
    } else if let Some(mobile) = &mobile {
        ("mobile_app", format!("{mobile} mobile app"))
    } else if let Some(meta) = &meta {
        ("web_app", format!("{meta} full-stack web app"))
    } else if let (Some(front), Some(back)) = (&frontend, &backend) {
        if matches!(front.as_str(), "Livewire" | "Inertia") {
            ("web_app", format!("{back} {} full-stack app with {front}", backend_language(back)))
        } else {
            ("web_app", format!("{front} single-page app with a {back} {} backend", backend_language(back)))
        }
    } else if let Some(front) = &frontend {
        let flavor = if typescript { " (TypeScript)" } else { "" };
        ("frontend", format!("{front} single-page app{flavor}"))
    } else if let Some(back) = &backend {
        ("backend", format!("{back} {} backend", backend_language(back)))
    } else if is_cli {
        let language = primary.clone().unwrap_or_else(|| "Command-line".to_owned());
        ("cli", format!("{language} command-line tool"))
    } else if let Some(language) = &primary {
        ("project", format!("{language} project"))
    } else {
        return None;
    };

    Some(RepoStack { kind: kind.to_owned(), summary, technologies: found })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::ArtifactType;

    fn file(path: &str, language: &str, size: i64) -> RepoFile {
        RepoFile {
            path: path.to_owned(),
            artifact_id: path.to_owned(),
            artifact_type: ArtifactType::CodeFile,
            language: Some(language.to_owned()),
            size_bytes: size,
            indexed: true,
            index_failure: None,
            commit_sha: "abc".to_owned(),
        }
    }

    fn manifests(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries.iter().map(|(path, text)| ((*path).to_owned(), (*text).to_owned())).collect()
    }

    #[test]
    fn detects_a_react_single_page_app() {
        let files = vec![file("src/App.tsx", "TypeScript", 900)];
        let stack = detect_stack(
            &files,
            &manifests(&[("package.json", r#"{"dependencies":{"react":"18"},"devDependencies":{"vite":"5","typescript":"5"}}"#)]),
        )
        .unwrap();
        assert_eq!(stack.kind, "frontend");
        assert_eq!(stack.summary, "React single-page app (TypeScript)");
        assert!(stack.technologies.iter().any(|technology| technology.name == "Vite"));
    }

    #[test]
    fn detects_a_laravel_backend() {
        let files = vec![file("artisan", "PHP", 10), file("app/Http/Controller.php", "PHP", 500)];
        let stack = detect_stack(&files, &manifests(&[("composer.json", r#"{"require":{"laravel/framework":"^11"}}"#)])).unwrap();
        assert_eq!(stack.kind, "backend");
        assert_eq!(stack.summary, "Laravel PHP backend");
    }

    #[test]
    fn detects_a_tauri_app_with_a_rust_backend() {
        let files = vec![file("src/main.rs", "Rust", 900), file("web/App.tsx", "TypeScript", 400)];
        let stack = detect_stack(
            &files,
            &manifests(&[
                ("Cargo.toml", "[dependencies]\ntauri = \"2\"\naxum = \"0.8\"\naxum-extra = \"0.9\""),
                ("web/package.json", r#"{"dependencies":{"react":"18"}}"#),
            ]),
        )
        .unwrap();
        assert_eq!(stack.kind, "desktop_app");
        assert_eq!(stack.summary, "Tauri desktop app with a React interface and a Axum Rust backend");
    }

    #[test]
    fn matches_whole_dependency_names_only() {
        assert!(mentions("axum = \"1\"", "axum"));
        assert!(!mentions("axum-extra = \"1\"", "axum"));
        assert!(!mentions("flaxum = \"1\"", "axum"));
    }

    #[test]
    fn nothing_identifiable_gives_no_stack() {
        assert!(detect_stack(&[file("notes.md", "Markdown", 10)], &BTreeMap::new()).is_none());
    }
}
