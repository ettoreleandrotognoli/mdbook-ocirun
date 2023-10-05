use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const SUCCESS_PATH: &str = "success.txt";
const ERROR_PATH: &str = "error.txt";

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct Config {
    pub image: String,
    pub command: String,
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
}

pub struct CodeSnippet {
    pub source: Source,
    pub input: Option<Source>,
    pub expected: Option<Result<Source, Source>>,
    pub config: Config,
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
            snippet.config.image, snippet.config.command
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

#[cfg(test)]
mod tests {
    use super::{CodeSnippet, CodeSnippetCache, Config, Source};

    #[test]
    pub fn test_cache() {
        let snippet = CodeSnippet {
            config: Config {
                image: "alpine".to_string(),
                command: "ash".to_string(),
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
}
