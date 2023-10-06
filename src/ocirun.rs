use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

use mdbook::book::Book;
use mdbook::book::Chapter;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};

use crate::utils::map_chapter;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LangConfig {
    pub name: String,
    pub image: String,
    pub command: Vec<String>,
}

impl LangConfig {
    pub fn rust() -> Self {
        Self {
            name: "rust".into(),
            image: "rust".into(),
            command: vec![
                "/bin/bash".into(),
                "-ec".into(),
                "rustc source -o binary && ./binary < input".into(),
            ],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
pub struct OciRunConfig {
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub langs: Vec<LangConfig>,
}

impl OciRunConfig {
    pub fn create_preprocessor(&self, root_path: PathBuf) -> OciRun {
        OciRun {
            engine: match &self.engine {
                Some(engine) => engine.clone(),
                None => "docker".to_string(),
            },
            root_path,
            langs: self.langs.clone(),
        }
    }
}

pub struct OciRun {
    pub engine: String,
    pub root_path: PathBuf,
    pub langs: Vec<LangConfig>,
}

impl Default for OciRun {
    fn default() -> Self {
        OciRunConfig::default().create_preprocessor(Path::new(".").to_path_buf())
    }
}

lazy_static! {
    static ref OCIRUN_REG_NEWLINE: Regex = Regex::new(r"<!--[ ]*ocirun (.*?)-->\r?\n")
        .expect("Failed to init regex for finding newline pattern");
    static ref OCIRUN_REG_INLINE: Regex = Regex::new(r"<!--[ ]*ocirun (.*?)-->")
        .expect("Failed to init regex for finding inline pattern");
}

const LAUNCH_SHELL_COMMAND: &str = "sh";
const LAUNCH_SHELL_FLAG: &str = "-c";

impl Preprocessor for OciRun {
    fn name(&self) -> &str {
        "ocirun"
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }

    fn run(&self, context: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let key = format!("preprocessor.{}", self.name());
        let config = context
            .config
            .get_deserialized_opt::<OciRunConfig, _>(key)
            .with_context(|| "Could not deserialize [preprocessor.ocirun]")
            .unwrap()
            .unwrap_or(OciRunConfig::default());
        let preprocessor = config.create_preprocessor(context.root.clone());
        map_chapter(&mut book, &mut move |chapter| {
            preprocessor.run_on_chapter(chapter)
        })?;
        Ok(book)
    }
}

lazy_static! {
    static ref SRC_DIR: String = get_src_dir();
}

#[derive(Deserialize)]
struct BookConfig {
    book: BookField,
}

#[derive(Deserialize)]
struct BookField {
    src: Option<String>,
}

fn get_src_dir() -> String {
    fs::read_to_string(Path::new("book.toml"))
        .map_err(|_| None::<String>)
        .and_then(|fc| toml::from_str::<BookConfig>(fc.as_str()).map_err(|_| None))
        .and_then(|bc| bc.book.src.ok_or(None))
        .unwrap_or_else(|_| String::from("src"))
}

impl OciRun {
    fn run_on_chapter(&self, chapter: &mut Chapter) -> Result<()> {
        let working_dir = &chapter
            .path
            .to_owned()
            .and_then(|p| {
                Path::new(SRC_DIR.as_str())
                    .join(p)
                    .parent()
                    .map(PathBuf::from)
            })
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_default();

        chapter.content = self.run_on_content(&chapter.content, working_dir)?;

        Ok(())
    }

    // This method is public for regression tests
    pub fn run_on_content(&self, content: &str, working_dir: &str) -> Result<String> {
        let mut err = None;

        let mut result = OCIRUN_REG_NEWLINE
            .replace_all(content, |caps: &Captures| {
                self.run_ocirun(caps[1].to_string(), working_dir, false)
                    .unwrap_or_else(|e| {
                        err = Some(e);
                        String::new()
                    })
            })
            .to_string();

        if let Some(e) = err {
            return Err(e);
        }

        result = OCIRUN_REG_INLINE
            .replace_all(result.as_str(), |caps: &Captures| {
                self.run_ocirun(caps[1].to_string(), working_dir, true)
                    .unwrap_or_else(|e| {
                        err = Some(e);
                        String::new()
                    })
            })
            .to_string();

        result = self.run_snippets_of_content(result.as_str()).unwrap();

        match err {
            None => Ok(result),
            Some(err) => Err(err),
        }
    }

    // Some progams output linebreaks in UNIX format,
    // this can cause problems on Windows if for any reason
    // the user is expecting consistent linebreaks,
    // e.g. they run the resulting markdown through a validation tool.
    //
    // So this function simply replaces all linebreaks with Windows linebreaks.
    #[cfg(target_family = "windows")]
    fn format_whitespace(str: Cow<'_, str>, inline: bool) -> String {
        let str = match inline {
            // When running inline it is undeseriable to have trailing whitespace
            true => str.trim_end(),
            false => str.as_ref(),
        };

        let mut res = str.lines().collect::<Vec<_>>().join("\r\n");
        if !inline && !res.is_empty() {
            res.push_str("\r\n");
        }

        return res;
    }

    #[cfg(any(target_family = "unix", target_family = "other"))]
    pub fn format_whitespace(str: Cow<'_, str>, inline: bool) -> String {
        match inline {
            // Wh;n running inline it is undeseriable to have trailing whitespace
            true => str.trim_end().to_string(),
            false => str.to_string(),
        }
    }

    // This method is public for unit tests
    pub fn run_ocirun(
        &self,
        raw_command: String,
        working_dir: &str,
        inline: bool,
    ) -> Result<String> {
        let absolute_working_dir = Path::new(working_dir).canonicalize().unwrap();
        //let output = Command::new(LAUNCH_SHELL_COMMAND)
        //    .args([LAUNCH_SHELL_FLAG, &command])
        //    .current_dir(working_dir)
        //    .output()
        //    .with_context(|| "Fail to run shell")?;
        let (image, cmd) = raw_command
            .split_once(' ')
            .unwrap_or(("alpine", raw_command.as_str()));
        let mut command = Command::new(self.engine.as_str());
        command.stdin(Stdio::null()).args([
            "run",
            "--rm",
            "-w",
            absolute_working_dir.to_str().unwrap(),
            "-v",
            format!("{0:}:{0:}", absolute_working_dir.to_str().unwrap()).as_str(),
            "-t",
            image,
            LAUNCH_SHELL_COMMAND,
            LAUNCH_SHELL_FLAG,
            cmd,
        ]);
        eprintln!(">>>>>>>>> {:?}", &command);

        let output = command.output().with_context(|| "Fail to run shell")?;

        eprintln!(">>>>>>>>> {:?}", &output);

        let stdout = Self::format_whitespace(String::from_utf8_lossy(&output.stdout), inline)
            .replace("\r\n", "\n");

        // let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // eprintln!("command: {}", command);
        // eprintln!("stdout: {:?}", stdout);
        // eprintln!("stderr: {:?}", stderr);

        Ok(stdout)
    }
}

#[cfg(test)]
mod tests {
    use crate::{ocirun::LangConfig, OciRunConfig};

    #[test]
    pub fn test_deserialize_config() {
        let expected = OciRunConfig {
            engine: Some("podman".into()),
            langs: vec![LangConfig::rust(),LangConfig::rust()],
        };
        let toml_config = r#"
        engine = "podman"
        [[langs]]
        name = "rust"
        image = "rust"
        command = ["/bin/bash", "-ec", "rustc source -o binary && ./binary < input"]
        [[langs]]
        name = "rust"
        image = "rust"
        command = ["/bin/bash", "-ec", "rustc source -o binary && ./binary < input"]
        "#;
        let config: OciRunConfig = toml::from_str(toml_config).unwrap();
        assert_eq!(config, expected);
    }
}
