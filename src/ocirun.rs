use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use cfg_if::cfg_if;
use lazy_static::lazy_static;
use regex::Captures;
use regex::Regex;
use serde::Deserialize;

use mdbook::book::Book;
use mdbook::book::Chapter;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};

use crate::utils::map_chapter;

pub struct OciRun;

lazy_static! {
    static ref CMDRUN_REG_NEWLINE: Regex = Regex::new(r"<!--[ ]*ocirun (.*?)-->\r?\n")
        .expect("Failed to init regex for finding newline pattern");
    static ref CMDRUN_REG_INLINE: Regex = Regex::new(r"<!--[ ]*ocirun (.*?)-->")
        .expect("Failed to init regex for finding inline pattern");
}

cfg_if! {
    if #[cfg(any(target_family = "unix", target_family = "other"))] {
        const LAUNCH_SHELL_COMMAND: &str = "sh";
        const LAUNCH_SHELL_FLAG: &str = "-c";
    } else if #[cfg(target_family = "windows")] {
        const LAUNCH_SHELL_COMMAND: &str = "cmd";
        const LAUNCH_SHELL_FLAG: &str = "/C";
    }
}

impl Preprocessor for OciRun {
    fn name(&self) -> &str {
        "ocirun"
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        map_chapter(&mut book, &mut OciRun::run_on_chapter)?;

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
    fn run_on_chapter(chapter: &mut Chapter) -> Result<()> {
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

        chapter.content = OciRun::run_on_content(&chapter.content, working_dir)?;

        Ok(())
    }

    // This method is public for regression tests
    pub fn run_on_content(content: &str, working_dir: &str) -> Result<String> {
        let mut err = None;

        let mut result = CMDRUN_REG_NEWLINE
            .replace_all(content, |caps: &Captures| {
                Self::run_ocirun(caps[1].to_string(), working_dir, false).unwrap_or_else(|e| {
                    err = Some(e);
                    String::new()
                })
            })
            .to_string();

        if let Some(e) = err {
            return Err(e);
        }

        result = CMDRUN_REG_INLINE
            .replace_all(result.as_str(), |caps: &Captures| {
                Self::run_ocirun(caps[1].to_string(), working_dir, true).unwrap_or_else(|e| {
                    err = Some(e);
                    String::new()
                })
            })
            .to_string();

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
    fn format_whitespace(str: Cow<'_, str>, inline: bool) -> String {
        match inline {
            // Wh;n running inline it is undeseriable to have trailing whitespace
            true => str.trim_end().to_string(),
            false => str.to_string(),
        }
    }

    // This method is public for unit tests
    pub fn run_ocirun(raw_command: String, working_dir: &str, inline: bool) -> Result<String> {
        let absolute_working_dir = Path::new(working_dir).canonicalize().unwrap();
        //let output = Command::new(LAUNCH_SHELL_COMMAND)
        //    .args([LAUNCH_SHELL_FLAG, &command])
        //    .current_dir(working_dir)
        //    .output()
        //    .with_context(|| "Fail to run shell")?;
        let image_and_command = raw_command
            .split_once(" ")
            .or(Some(("alpine", raw_command.as_str())))
            .unwrap();
        let image = image_and_command.0;
        let mut command = Command::new("docker");
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
            &image_and_command.1,
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
