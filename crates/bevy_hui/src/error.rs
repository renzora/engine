use owo_colors::OwoColorize;
use std::fmt::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("failed to read bytes to end `{0}`")]
    FailedToRead(String),

    #[error("It's not utf8, what are you doing?")]
    Utf8Error,

    #[error("{0}")]
    Nom(String),

    #[error("Failed with incomplete data")]
    Incomplete,
}

pub enum HtmlError<'a> {
    Tag(&'a [u8], nom::error::ErrorKind),
    Ctx(&'a [u8], &'static str),
}

pub struct VerboseHtmlError<'a> {
    trace: Vec<HtmlError<'a>>,
}

impl std::fmt::Debug for VerboseHtmlError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let err_str = self.format(b"", "");
        write!(f, "{}", err_str)
    }
}

#[allow(unused_must_use)]
impl<'a> VerboseHtmlError<'a> {
    pub fn format(&'a self, source: &'a [u8], file: &str) -> String {
        let mut out = String::new();

        write!(
            &mut out,
            "\n{}",
            "------------------------------------".green()
        );

        for err in self.trace.iter() {
            match err {
                HtmlError::Tag(input, error_kind) => {
                    let line_num = get_line_num(&source, input);
                    write!(
                        &mut out,
                        "\n{}[{}] of kind {:?}",
                        "[HTML ERROR]".red(),
                        line_num.green(),
                        error_kind.red()
                    );
                }
                HtmlError::Ctx(input, ctx) => {
                    let (before, after, line_num) = get_line_parts_and_num(&source, input);

                    write!(
                        &mut out,
                        "\n{}[{}] `{}`",
                        "[HTML ERROR]".red(),
                        line_num.green(),
                        ctx.red()
                    );
                    write!(&mut out, "\n[in `{}` at line {}]:", file, line_num.green());
                    write!(
                        &mut out,
                        "\n{}{}{}\n",
                        std::str::from_utf8(before).unwrap_or_default().trim_start(),
                        std::str::from_utf8(input).unwrap_or_default().green(),
                        std::str::from_utf8(after).unwrap_or_default().trim_end(),
                    );
                }
            }
        }

        write!(
            &mut out,
            "\n{}\n",
            "------------------------------------".green()
        );
        out
    }
}

impl<'a> nom::error::ParseError<&'a [u8]> for VerboseHtmlError<'a> {
    fn from_error_kind(input: &'a [u8], kind: nom::error::ErrorKind) -> Self {
        Self {
            trace: vec![HtmlError::Tag(input, kind)],
        }
    }

    fn append(input: &'a [u8], kind: nom::error::ErrorKind, mut other: Self) -> Self {
        other.trace.push(HtmlError::Tag(input, kind));
        other
    }
}

impl<'a> nom::error::ContextError<&'a [u8]> for VerboseHtmlError<'a> {
    fn add_context(input: &'a [u8], ctx: &'static str, mut other: Self) -> Self {
        other.trace.push(HtmlError::Ctx(input, ctx));
        other
    }
}

fn get_line_num(source: &[u8], slice: &[u8]) -> u32 {
    let start = (slice.as_ptr() as usize) - (source.as_ptr() as usize);
    let start_index = start / std::mem::size_of::<u8>();
    let preceding_source = &source[..start_index];
    preceding_source.iter().filter(|&&c| c == b'\n').count() as u32 + 1
}

//dirty af
fn get_line_parts_and_num<'a>(source: &'a [u8], slice: &'a [u8]) -> (&'a [u8], &'a [u8], u32) {
    let start = (slice.as_ptr() as usize) - (source.as_ptr() as usize);
    let start_index = start / std::mem::size_of::<u8>();

    let line_start = source[..start_index]
        .iter()
        .rposition(|&c| c == b'\n')
        .map_or(0, |pos| pos + 1);

    let line_end = source[start_index..]
        .iter()
        .position(|&c| c == b'\n')
        .map_or(source.len(), |pos| start_index + pos);

    let line = &source[line_start..line_end];

    let slice_start_in_line = start_index - line_start;
    let slice_end_in_line = slice_start_in_line + slice.len();

    let first = &line[..slice_start_in_line];
    let second = &line[slice_end_in_line..];

    let line_number = source[..start_index]
        .iter()
        .filter(|&&c| c == b'\n')
        .count() as u32
        + 1;

    (first, second, line_number)
}
