/**
 * Definitions of data structures used by several modules, such as `QuizError` and the
 * various structs that hold command-line arguments.
 *
 * Author:  Ian Fisher (iafisher@fastmail.com)
 * Version: October 2019
 */
use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

pub type Result<T> = ::std::result::Result<T, QuizError>;

#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum QuizError {
    /// For when the user requests a quiz that does not exist.
    QuizNotFound(PathBuf),
    /// For JSON errors.
    Json(serde_json::Error),
    CannotWriteToFile(PathBuf),
    Io(io::Error),
    ReadlineInterrupted,
    EmptyQuiz,
    Parse {
        line: usize,
        whole_entry: bool,
        message: String,
    },
    CannotOpenEditor,
    /// Not really an error, but a signal sent when the user wants to mark their
    /// previous answer as correct.
    SignalMarkCorrect,
    /// A signal sent when the user wants to edit the previous question.
    SignalEdit,
}

impl fmt::Display for QuizError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            QuizError::QuizNotFound(ref path) => {
                write!(f, "could not find quiz named '{}'", path.to_string_lossy())
            }
            QuizError::Json(ref err) => write!(f, "could not parse JSON ({})", err),
            QuizError::CannotWriteToFile(ref path) => {
                write!(f, "could not write to file '{}'", path.to_string_lossy())
            }
            QuizError::Io(ref err) => write!(f, "IO error ({})", err),
            QuizError::EmptyQuiz => write!(f, "no questions found"),
            QuizError::ReadlineInterrupted => Ok(()),
            QuizError::Parse {
                line,
                whole_entry,
                ref message,
            } => {
                let location = if !whole_entry {
                    format!("on line {}", line)
                } else {
                    format!("in entry beginning on line {}", line)
                };

                write!(f, "{} {}", message, location)
            }
            QuizError::CannotOpenEditor => write!(f, "unable to open text editor"),
            QuizError::SignalMarkCorrect => write!(f, "internal error ('SignalMarkCorrect')"),
            QuizError::SignalEdit => write!(f, "internal error ('SignalEdit')"),
        }
    }
}

impl error::Error for QuizError {
    fn cause(&self) -> Option<&error::Error> {
        match *self {
            QuizError::Json(ref err) => Some(err),
            _ => None,
        }
    }
}

/// Holds the command-line configuration for the application.
pub struct Options {
    /// Do not emit colorized output.
    pub no_color: bool,
    pub cmd: Command,
}

pub enum Command {
    History(HistoryOptions),
    Results(ResultsOptions),
    Search(SearchOptions),
    Take(TakeOptions),
}

/// These filtering options are shared between the `take` and `count` subcommands.
pub struct FilterOptions {
    pub exclude: Vec<String>,
    pub tags: Vec<String>,
}

pub struct HistoryOptions {
    pub name: PathBuf,
    pub id: String,
}

pub struct ResultsOptions {
    pub name: PathBuf,
    pub num_to_show: Option<usize>,
    pub sort: String,
}

pub struct SearchOptions {
    pub name: PathBuf,
    pub term: String,
    pub filter_opts: FilterOptions,
}

pub struct TakeOptions {
    /// Name of the quiz to take.
    pub name: PathBuf,
    pub flip: bool,
    pub in_order: bool,
    pub no_save: bool,
    pub num_to_ask: usize,
    pub random: bool,
    pub filter_opts: FilterOptions,
}

/// Return `true` if `tags` satisfies the constraints in `options`.
pub fn filter_tags(tags: &Vec<String>, options: &FilterOptions) -> bool {
    // Either no tags were specified, or `q` has at least one of the specified tags.
    (options.tags.len() == 0 || options.tags.iter().any(|tag| tags.contains(tag)))
        // `q` must not have any excluded tags.
        && options.exclude.iter().all(|tag| !tags.contains(tag))
}
