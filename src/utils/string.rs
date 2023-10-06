use std::borrow::Cow;
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
