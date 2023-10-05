use std::{
    env::temp_dir,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context;

use crate::OciRun;

const SUCCESS_PATH: &str = "success.txt";
const ERROR_PATH: &str = "error.txt";

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct Config<'a> {
    pub image: &'a str,
    pub command: Vec<&'a str>,
}

pub enum Source {
    File(String),
    String(String),
}

impl Source {
    fn get_content(&self) -> String {
        match self {
            Self::String(content) => content.clone(),
            Self::File(file) => std::fs::read_to_string(file).unwrap(),
        }
    }

    fn get_digest(&self) -> String {
        sha256::digest(self.get_content())
    }

    fn get_path(&self) -> PathBuf {
        match self {
            Self::String(content) => {
                let path = temp_dir().join(self.get_digest());
                std::fs::write(path.clone(), content).unwrap();
                path
            }
            Self::File(file) => Path::new(file).to_path_buf(),
        }
    }
}

pub struct CodeSnippet<'a> {
    pub source: Source,
    pub input: Option<Source>,
    pub expected: Option<Result<Source, Source>>,
    pub config: Config<'a>,
}

struct CodeSnippetCache {
    pub path: String,
}

impl Default for CodeSnippetCache {
    fn default() -> Self {
        let home = home::home_dir().unwrap().canonicalize().unwrap();
        let cache = format!("{}/.mdbook/ocirun/", home.to_str().unwrap());
        Self::new(cache)
    }
}

impl CodeSnippetCache {
    fn new(path: String) -> Self {
        let cache = Path::new(path.as_str());
        if !cache.is_dir() {
            std::fs::create_dir_all(&path).unwrap();
        }
        Self { path }
    }

    #[cfg(test)]
    fn temp() -> Self {
        let temp = std::env::temp_dir();
        let cache = format!("{}/.mdbook/ocirun/", temp.to_str().unwrap());
        Self::new(cache)
    }

    #[cfg(test)]
    fn clear(&self) {
        let path = Path::new(self.path.as_str());
        std::fs::remove_dir_all(path).unwrap();
    }

    fn as_cached_path(&self, snippet: &CodeSnippet) -> PathBuf {
        let config_path = sha256::digest(format!(
            "{}:{}",
            snippet.config.image,
            snippet.config.command.join(" ")
        ));
        let source_hash = snippet.source.get_digest();
        let mut cache_path = Path::new(self.path.as_str())
            .join(config_path)
            .join(source_hash);
        if let Some(input) = &snippet.input {
            let input_hash = input.get_digest();
            cache_path = cache_path.join(input_hash);
        }
        cache_path
    }

    fn get(&self, snippet: &CodeSnippet) -> Option<Result<String, String>> {
        let cache_path = self.as_cached_path(snippet);
        if !cache_path.is_dir() {
            return None;
        }
        let success_output = cache_path.join(Path::new(SUCCESS_PATH));
        if success_output.exists() {
            let content = std::fs::read_to_string(success_output).unwrap();
            return Some(Ok(content));
        }
        let error_output = cache_path.join(Path::new(ERROR_PATH));
        if error_output.exists() {
            let content = std::fs::read_to_string(error_output).unwrap();
            return Some(Err(content));
        }
        None
    }

    fn add(&self, snippet: &CodeSnippet, result: &Result<String, String>) {
        let cache_path = self.as_cached_path(snippet);
        let error_path = cache_path.join(ERROR_PATH);
        let success_path = cache_path.join(SUCCESS_PATH);
        std::fs::create_dir_all(cache_path).unwrap();
        let (file, content) = match result {
            Ok(content) => (File::create(success_path), content),
            Err(content) => (File::create(error_path), content),
        };
        file.unwrap().write_all(content.as_bytes()).unwrap();
    }
}

pub trait SnippetRunner {
    fn run(&self, snippet: &CodeSnippet) -> Result<String, String>;
}

struct CachedRunner<R: SnippetRunner> {
    cache: CodeSnippetCache,
    runner: R,
}

impl<R: SnippetRunner> SnippetRunner for CachedRunner<R> {
    fn run(&self, snippet: &CodeSnippet) -> Result<String, String> {
        if let Some(result) = self.cache.get(snippet) {
            return result;
        }
        let result = self.runner.run(snippet);
        self.cache.add(snippet, &result);
        result
    }
}

impl SnippetRunner for OciRun {
    fn run(&self, snippet: &CodeSnippet) -> Result<String, String> {
        let mut args = vec!["create", "-w", "/root", "-t", snippet.config.image];
        for &arg in &snippet.config.command {
            args.push(arg);
        }

        let container_id = Command::new(self.engine.as_str())
            .stdin(Stdio::null())
            .args(args)
            .output()
            .with_context(|| "Fail to create container")
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim_end()
                    .to_string()
            })
            .unwrap();

        let source_path = snippet.source.get_path();
        let container_file = format!("{}:/root/source", container_id);
        let args = vec!["cp", source_path.to_str().unwrap(), container_file.as_str()];
        let _copy_source_result = Command::new(self.engine.as_str())
            .stdin(Stdio::null())
            .args(args)
            .output()
            .with_context(|| "Fail to copy source")
            .unwrap();

        let input_path = match &snippet.input {
            Some(source) => source.get_path(),
            None => Path::new("/dev/null").to_path_buf(),
        };
        let container_file = format!("{}:/root/input", container_id);
        let args = vec!["cp", input_path.to_str().unwrap(), container_file.as_str()];
        let _copy_input_result = Command::new(self.engine.as_str())
            .stdin(Stdio::null())
            .args(args)
            .output()
            .with_context(|| "Fail to copy input")
            .unwrap();

        let args = vec!["start", "-a", container_id.as_str()];

        let output = Command::new(self.engine.as_str())
            .stdin(Stdio::null())
            .args(args)
            .output()
            .with_context(|| "Fail to run container")
            .unwrap();

        let stdout = Self::format_whitespace(String::from_utf8_lossy(&output.stdout), false)
            .replace("\r\n", "\n");

        match output.status.success() {
            true => Ok(stdout),
            false => Err(stdout),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::ocirun::OciRunConfig;

    use super::{CodeSnippet, CodeSnippetCache, Config, SnippetRunner, Source};

    #[test]
    pub fn test_cache() {
        let snippet = CodeSnippet {
            config: Config {
                image: "alpine",
                command: vec!["ash"],
            },
            input: None,
            expected: None,
            source: Source::String("echo ok".to_string()),
        };
        let cache = CodeSnippetCache::temp();
        let expected: Result<String, String> = Result::Ok("ok".to_string());
        let none = cache.get(&snippet);
        assert_eq!(none, None);
        cache.add(&snippet, &expected);
        let result = cache.get(&snippet).unwrap();
        assert_eq!(result, expected);
        cache.clear();
    }

    #[test]
    pub fn test_run_snippet() {
        let config = OciRunConfig {
            ..Default::default()
        };
        let runner = config.create_preprocessor(Path::new(".").to_path_buf());
        let snippet = CodeSnippet {
            source: Source::String(
                r#"
                fn main() {
                    println!("Hello World!!!");
                }
            "#
                .into(),
            ),
            input: None,
            expected: None,
            config: Config {
                image: "rust",
                command: vec![
                    "/bin/bash",
                    "-ec",
                    "rustc source -o binary && ./binary < input",
                ],
            },
        };
        let result = runner.run(&snippet);
        assert_eq!(result, Result::Ok("Hello World!!!\n".into()));
    }
}
