/**
 * Definitions of data structures used by several modules, such as `QuizError` and the
 * various structs that hold command-line arguments.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use structopt::StructOpt;


#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum QuizError {
    /// For when the user requests a quiz that does not exist.
    QuizNotFound(String),
    /// For JSON errors.
    Json(serde_json::Error),
    /// For when the user's system editor cannot be opened.
    CannotOpenEditor,
    CannotRunGit,
    CannotWriteToFile(PathBuf),
    Io(io::Error),
    ReadlineInterrupted,
    EmptyQuiz,
    Parse { line: usize, whole_entry: bool },
}


impl fmt::Display for QuizError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            QuizError::QuizNotFound(ref name) => {
                write!(f, "could not find quiz named '{}'", name)
            },
            QuizError::Json(ref err) => {
                write!(f, "could not parse JSON ({})", err)
            },
            QuizError::CannotOpenEditor => {
                write!(f, "unable to open system editor")
            },
            QuizError::CannotRunGit => {
                write!(f, "unable to run git (is it installed and on the PATH?)")
            },
            QuizError::CannotWriteToFile(ref path) => {
                write!(f, "cannot write to file '{}'", path.to_string_lossy())
            },
            QuizError::Io(ref err) => {
                write!(f, "IO error ({})", err)
            },
            QuizError::EmptyQuiz => {
                write!(f, "no questions found")
            },
            QuizError::ReadlineInterrupted => {
                Ok(())
            },
            QuizError::Parse { line, whole_entry } => {
                if !whole_entry {
                    write!(f, "parse error on line {}", line)
                } else {
                    write!(f, "parse error in entry beginning on line {}", line)
                }
            }
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
#[derive(StructOpt)]
#[structopt(name = "popquiz", about = "Take quizzes from the command line.")]
pub enum QuizOptions {
    /// Take a quiz.
    #[structopt(name = "take")]
    Take(QuizTakeOptions),
    /// Count questions or tags.
    #[structopt(name = "count")]
    Count(QuizCountOptions),
    /// Report results of previous attempts.
    #[structopt(name = "results")]
    Results(QuizResultsOptions),
    /// Edit or create a quiz.
    #[structopt(name = "edit")]
    Edit(QuizEditOptions),
    /// Delete a quiz.
    #[structopt(name = "rm")]
    Rm(QuizRmOptions),
    /// Rename a quiz.
    #[structopt(name = "mv")]
    Mv(QuizMvOptions),
    /// List all available quizzes.
    #[structopt(name = "ls")]
    Ls(QuizLsOptions),
    /// Print file paths of quizzes.
    #[structopt(name = "path")]
    Path(QuizPathOptions),
    /// Seach questions for a keyword.
    #[structopt(name = "search")]
    Search(QuizSearchOptions),
    /// Invoke git in the quiz folder.
    #[structopt(name = "git")]
    Git { args: Vec<String> },
}

#[derive(StructOpt)]
pub struct QuizTakeOptions {
    /// Name of the quiz to take.
    #[structopt(default_value = "main")]
    pub name: String,
    /// Limit the total number of questions.
    #[structopt(short = "n")]
    pub num_to_ask: Option<usize>,
    /// Choose from the `n` questions with the highest previous scores.
    #[structopt(long = "best")]
    pub best: Option<usize>,
    /// Choose from the `n` questions with the lowest previous scores.
    #[structopt(long = "worst")]
    pub worst: Option<usize>,
    /// Choose from the `n` questions with the most previous attempts.
    #[structopt(long = "most")]
    pub most: Option<usize>,
    /// Choose from the `n` questions with the least previous attempts.
    #[structopt(long = "least")]
    pub least: Option<usize>,
    /// Save results without prompting.
    #[structopt(long = "save")]
    pub save: bool,
    /// Do not emit colorized output.
    #[structopt(long = "no-color")]
    pub no_color: bool,
    /// Ask the questions in the order they appear in the quiz file.
    #[structopt(long = "in-order")]
    pub in_order: bool,
    /// Flip flashcards.
    #[structopt(long = "flip")]
    pub flip: bool,
    #[structopt(flatten)]
    pub filter_opts: QuizFilterOptions,
}

#[derive(StructOpt)]
pub struct QuizCountOptions {
    /// Name of the quiz to count.
    #[structopt(default_value = "main")]
    pub name: String,
    /// List tags instead of counting questions.
    #[structopt(long = "list-tags")]
    pub list_tags: bool,
    #[structopt(flatten)]
    pub filter_opts: QuizFilterOptions,
}

/// These filtering options are shared between the `take` and `count` subcommands.
#[derive(StructOpt)]
pub struct QuizFilterOptions {
    /// Only include questions with the given tag.
    #[structopt(long = "tag")]
    pub tags: Vec<String>,
    /// Exclude questions with the given tag.
    #[structopt(long = "exclude")]
    pub exclude: Vec<String>,
    /// Only include questions that have never been asked before.
    #[structopt(long = "never")]
    pub never: bool,
}

#[derive(StructOpt)]
pub struct QuizEditOptions {
    /// The name of the quiz to edit.
    #[structopt(default_value = "main")]
    pub name: String,
    /// Edit the results file rather than the quiz itself.
    #[structopt(short = "r", long = "results")]
    pub results: bool,
}

#[derive(StructOpt)]
pub struct QuizRmOptions {
    /// The name of the quiz to delete.
    #[structopt(default_value = "main")]
    pub name: String,
    /// Delete without prompting for confirmation.
    #[structopt(short = "f", long = "force")]
    pub force: bool,
}

#[derive(StructOpt)]
pub struct QuizMvOptions {
    /// The old name of the quiz to rename.
    pub old_name: String,
    /// The new name.
    pub new_name: String,
}

#[derive(StructOpt)]
pub struct QuizResultsOptions {
    /// The name of the quiz for which to fetch the results.
    #[structopt(default_value = "main")]
    pub name: String,
    /// One of 'best', 'worst', 'most' or 'least'. Defaults to 'best'.
    #[structopt(short = "s", long = "sort", default_value = "best")]
    pub sort: String,
    /// Only show the first `n` results.
    #[structopt(short = "n")]
    pub num_to_show: Option<usize>,
}


#[derive(StructOpt)]
pub struct QuizLsOptions {
    /// List quizzes whose name begins with a period.
    #[structopt(short = "a", long = "all")]
    pub all: bool,
}


#[derive(StructOpt)]
pub struct QuizPathOptions {
    /// The name of the quiz.
    #[structopt(default_value = "main")]
    pub name: String,
    /// Show the path to the results file instead of the quiz file.
    #[structopt(short = "r", long = "results")]
    pub results: bool,
    /// Display the path that would be used even if the quiz does not exist.
    #[structopt(short = "f", long = "force")]
    pub force: bool,
}


#[derive(StructOpt)]
pub struct QuizSearchOptions {
    /// The name of the quiz.
    pub name: String,
    /// The term to search for.
    pub term: String,
}


impl QuizTakeOptions {
    #[allow(dead_code)]
    pub fn new() -> Self {
        QuizTakeOptions {
            name: String::new(), num_to_ask: None, best: None, worst: None, most: None,
            least: None, save: false, no_color: true, in_order: false, flip: false,
            filter_opts: QuizFilterOptions::new()
        }
    }
}


impl QuizFilterOptions {
    #[allow(dead_code)]
    pub fn new() -> Self {
        QuizFilterOptions {
            tags: Vec::new(), exclude: Vec::new(), never: false,
        }
    }
}
