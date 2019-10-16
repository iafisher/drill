/**
 * Helper functions for input and output.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use colored::*;
use std::io::Write;

use rustyline::error::ReadlineError;

use super::quiz::QuizError;


#[macro_export]
macro_rules! my_println {
    ($($arg:tt)*) => (
        writeln!(std::io::stdout(), $($arg)*).map_err(QuizError::Io)
    );
}

#[macro_export]
macro_rules! my_print {
    ($($arg:tt)*) => (
        write!(std::io::stdout(), $($arg)*).map_err(QuizError::Io)
    );
}


/// Display a prompt and read a line from standard input continually until the user
/// enters a line with at least one non-whitespace character. If the user presses Ctrl+D
/// then `Ok(None)` is returned. If the user pressed Ctrl+C then `Err(())` is returned.
/// Otherwise, `Ok(Some(line))` is returned where `line` is the last line of input the
/// user entered without leading and trailing whitespace.
pub fn prompt(message: &str) -> Result<Option<String>, QuizError> {
    let mut rl = rustyline::Editor::<()>::new();
    loop {
        let result = rl.readline(&format!("{}", message.white()));
        match result {
            Ok(response) => {
                let response = response.trim();
                if response.len() > 0 {
                    return Ok(Some(response.to_string()));
                }
            },
            // Return immediately if the user hits Ctrl+D or Ctrl+C.
            Err(ReadlineError::Interrupted) => {
                return Err(QuizError::ReadlineInterrupted);
            },
            Err(ReadlineError::Eof) => {
                return Ok(None);
            },
            _ => {}
        }
    }
}


/// Prompt the user with a yes-no question and return `true` if they enter yes.
pub fn confirm(message: &str) -> bool {
    match prompt(message) {
        Ok(Some(response)) => {
            response.trim_start().to_lowercase().starts_with("y")
        },
        _ => false
    }
}


/// Print `message` to standard output, breaking lines according to the current width
/// of the terminal. If `prefix` is not `None`, then prepend it to the first line and
/// indent all subsequent lines by its length.
pub fn prettyprint(message: &str, prefix: Option<&str>) -> Result<(), QuizError> {
    prettyprint_colored(message, prefix, None, None)
}


pub fn prettyprint_colored(
    message: &str, prefix: Option<&str>, message_color: Option<Color>,
    prefix_color: Option<Color>
) -> Result<(), QuizError> {
    let prefix = prefix.unwrap_or("");
    let width = textwrap::termwidth() - prefix.len();
    let mut lines = textwrap::wrap_iter(message, width);

    if let Some(first_line) = lines.next() {
        let colored_prefix = color_optional(&prefix, prefix_color);
        let colored_line = color_optional(&first_line, message_color);
        my_println!("{}{}", colored_prefix, colored_line)?;
    }

    let indent = " ".repeat(prefix.len());
    for line in lines {
        let colored_line = color_optional(&line, message_color);
        my_println!("{}{}", indent, colored_line)?;
    }
    Ok(())
}


fn color_optional(text: &str, color: Option<Color>) -> ColoredString {
    if let Some(color) = color {
        text.color(color)
    } else {
        text.normal()
    }
}
