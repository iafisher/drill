/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: September 2019
 */
use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::os;

use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rustyline::error::ReadlineError;
use serde::{Serialize, Deserialize};
use structopt::StructOpt;

use super::parser;


/// Represents an entire quiz.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Quiz {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_kind: Option<String>,
    pub instructions: Option<String>,
    pub questions: Vec<Question>,
}


/// Represents a question.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub kind: QuestionKind,
    /// The text of the question. It is a vector instead of a string so that multiple
    /// variants of the same question can be stored.
    pub text: Vec<String>,
    /// Correct answers to the question. When `kind` is equal to `ShortAnswer` or
    /// `MultipleChoice`, this vector should have only one element.
    pub answer_list: Vec<Answer>,
    /// Candidate answers to the question. This field is only used when `kind` is set to
    /// `MultipleChoice`, in which case the candidates are incorrect answers to the
    /// question.
    #[serde(default)]
    pub candidates: Vec<String>,
    /// Prior results of answering the question.
    #[serde(default)]
    pub prior_results: Vec<QuestionResult>,
    /// User-defined tags for the question.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Incorrect answers may be given specific explanations for why they are not
    /// right.
    #[serde(default)]
    pub explanations: Vec<(Vec<String>, String)>,
}


/// An enumeration for the `kind` field of `Question` objects.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice, Flashcard,
}


/// Represents an answer.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    /// Each member of the `variants` vector should be an equivalent answer, e.g.
    /// `vec!["Mount Everest", "Everest"]`, not different answers to the same question.
    /// The first element of the vector is taken to be the canonical form of the answer
    /// for display.
    pub variants: Vec<String>,
}


/// Represents the result of answering a question on a particular occasion.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuestionResult {
    #[serde(skip)]
    text: String,
    time_asked: chrono::DateTime<chrono::Utc>,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<String>,
    /// If the question asked was a list question, then the user's responses go in this
    /// field.
    #[serde(skip_serializing_if = "Option::is_none")]
    response_list: Option<Vec<String>>,
    score: f64,

    // It would be convenient to include a reference to the `Question` object as a field
    // of this struct, but Rust's lifetimes makes it more difficult than it's worth.
}


impl PartialEq for QuestionResult {
    fn eq(&self, other: &QuestionResult) -> bool {
        self.time_asked == other.time_asked && self.response == other.response &&
            self.score == other.score
    }
}
impl Eq for QuestionResult {}


/// Represents the results of taking a quiz on a particular occasion.
#[derive(Debug)]
pub struct QuizResult {
    time_taken: chrono::DateTime<chrono::Utc>,
    total: usize,
    total_correct: usize,
    total_partially_correct: usize,
    total_incorrect: usize,
    score: f64,
    per_question: Vec<QuestionResult>,
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
    /// Filter by keyword.
    #[structopt(short = "k", long = "keyword")]
    pub keywords: Vec<String>,
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


// One main function for each subcommand.


/// The main function for the `take` subcommand.
pub fn main_take(options: QuizTakeOptions) -> Result<(), QuizError> {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = load_quiz(&options.name)?;
    let results = quiz.take(&options)?;
    output_results(&results)?;

    if results.total > 0 && (options.save || yesno("\nSave results? ")) {
        save_results(&options.name, &results)?;
    }
    Ok(())
}


fn output_results(results: &QuizResult) -> Result<(), QuizError> {
    if results.total > 0 {
        let score_as_str = format!("{:.1}%", results.score);

        my_print!("\n\n")?;
        my_print!("{}", "Score: ".white())?;
        my_print!("{}", score_as_str.cyan())?;
        my_print!("{}", " out of ".white())?;
        my_print!("{}", format!("{}", results.total).cyan())?;
        if results.total == 1 {
            my_println!("{}", " question".white())?;
        } else {
            my_println!("{}", " questions".white())?;
        }
        my_print!("  {}", format!("{}", results.total_correct).bright_green())?;
        my_print!("{}\n", " correct".white())?;
        my_print!("  {}", format!("{}", results.total_partially_correct).green())?;
        my_print!("{}\n", " partially correct".white())?;
        my_print!("  {}", format!("{}", results.total_incorrect).red())?;
        my_print!("{}\n", " incorrect".white())?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count(options: QuizCountOptions) -> Result<(), QuizError> {
    let quiz = load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(&quiz)?;
    } else {
        let filtered = quiz.filter_questions(&options.filter_opts);
        my_println!("{}", filtered.len())?;
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results(options: QuizResultsOptions) -> Result<(), QuizError> {
    let results = load_results(&options.name)?;

    if results.len() == 0 {
        my_println!("No results have been recorded for this quiz.")?;
        return Ok(());
    }

    let mut aggregated: Vec<(f64, usize, String)> = Vec::new();
    for (key, result) in results.iter() {
        // Only include questions that have scored results.
        if let Some(score) = aggregate_results(&result) {
            aggregated.push((score, result.len(), key.clone()));
        }
    }

    if options.sort == "best" {
        aggregated.sort_by(cmp_results_best);
    } else if options.sort == "worst" {
        aggregated.sort_by(cmp_results_worst);
    } else if options.sort == "most" {
        aggregated.sort_by(cmp_results_most);
    } else if options.sort == "least" {
        aggregated.sort_by(cmp_results_least);
    } else {
    }

    if let Some(n) = options.num_to_show {
        aggregated.truncate(n);
    }

    for (score, attempts, question) in aggregated.iter() {
        let first_prefix = format!("{:>5.1}%  of {:>2}   ", score, attempts);
        prettyprint_colored(&question, Some(&first_prefix), None, Some(Color::Cyan))?;
    }

    Ok(())
}


pub fn main_edit(options: QuizEditOptions) -> Result<(), QuizError> {
    let path = if options.results {
        get_results_path(&options.name)
    } else {
        get_quiz_path(&options.name)
    };

    loop {
        // Spawn an editor in a child process.
        let editor = ::std::env::var("EDITOR").unwrap_or(String::from("nano"));
        let mut child = Command::new(editor).arg(&path).spawn()
            .or(Err(QuizError::CannotOpenEditor))?;
        child.wait()
            .or(Err(QuizError::CannotOpenEditor))?;

        if !options.results && path.exists() {
            // Parse it again to make sure it's okay.
            if let Err(e) = parser::parse(&path) {
                eprintln!("{}: {}", "Error".red(), e);
                if !yesno("Do you want to save anyway? ") {
                    continue;
                }
            }
        }
        break;
    }

    if !options.results && path.exists() && is_git_repo() {
        git(&["add", &path.as_path().to_string_lossy()])?;
        git(&["commit", "-m", &format!("Edit {}", options.name)])?;
    }

    Ok(())
}


pub fn main_rm(options: QuizRmOptions) -> Result<(), QuizError> {
    let path = get_quiz_path(&options.name);
    if path.exists() {
        let yesno_prompt = "Are you sure you want to delete the quiz? ";
        if options.force || yesno(yesno_prompt) {
            fs::remove_file(&path).map_err(QuizError::Io)?;
        }

        if is_git_repo() {
            git(&["rm", &path.as_path().to_string_lossy()])?;
            git(&["commit", "-m", &format!("Remove {}", options.name)])?;
        }

        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.clone()))
    }
}


pub fn main_mv(options: QuizMvOptions) -> Result<(), QuizError> {
    let quiz_path = get_quiz_path(&options.old_name);
    let new_quiz_path = get_quiz_path(&options.new_name);
    fs::rename(&quiz_path, &new_quiz_path).map_err(QuizError::Io)?;

    let results_path = get_results_path(&options.old_name);
    let new_results_path = get_results_path(&options.new_name);
    if results_path.exists() {
        fs::rename(&results_path, &new_results_path).map_err(QuizError::Io)?;
    }

    if is_git_repo() {
        git(&["rm", &quiz_path.as_path().to_string_lossy()])?;
        git(&["add", &new_quiz_path.as_path().to_string_lossy()])?;
        git(
            &[
                "commit",
                "-m",
                &format!("Rename {} to {}", options.old_name, options.new_name)
            ]
        )?;
    }

    Ok(())
}


pub fn main_ls(options: QuizLsOptions) -> Result<(), QuizError> {
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");

    let mut quiz_names = Vec::new();
    if let Ok(iter) = dirpath.read_dir() {
        for entry in iter {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    // For example, a .git entry.
                    if file_type.is_dir() {
                        continue;
                    }
                }

                if let Some(stem) = entry.path().file_stem() {
                    quiz_names.push(String::from(stem.to_string_lossy()));
                }
            }
        }
    }

    if quiz_names.len() > 0 {
        quiz_names.sort_by(cmp_string_ignore_dot);
        my_println!("Available quizzes:")?;
        for name in quiz_names.iter() {
            if !name.starts_with(".") || options.all {
                my_println!("  {}", name)?;
            }
        }
    } else {
        my_println!("No quizzes found.")?;
    }

    Ok(())
}


pub fn main_path(options: QuizPathOptions) -> Result<(), QuizError> {
    let path = if options.results {
        get_results_path(&options.name)
    } else {
        get_quiz_path(&options.name)
    };

    if path.exists() || options.force {
        my_println!("{}", path.as_path().to_string_lossy())?;
        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.to_string()))
    }
}


pub fn main_git(args: Vec<String>) -> Result<(), QuizError> {
    let mut args_as_str = Vec::new();
    for arg in args.iter() {
        args_as_str.push(arg.as_str());
    }
    git(&args_as_str[..])
}


impl Quiz {
    /// Take the quiz and return pairs of questions and results.
    pub fn take(&mut self, options: &QuizTakeOptions) -> Result<QuizResult, QuizError> {
        if options.flip {
            for q in self.questions.iter_mut() {
                q.flip();
            }
        }

        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total_partially_correct = 0;
        let mut total = 0;
        let mut aggregate_score = 0.0;

        let questions = self.choose_questions(&options);
        if questions.len() == 0 {
            return Err(QuizError::EmptyQuiz);
        }

        if let Some(instructions) = &self.instructions {
            my_print!("\n")?;
            prettyprint_colored(
                &instructions, Some("  "), Some(Color::BrightBlue), None
            )?;
            my_print!("\n\n")?;
        }

        for (i, question) in questions.iter().enumerate() {
            my_print!("\n")?;
            let result = question.ask(i+1);
            if let Ok(result) = result {
                let score = result.score;
                results.push(result);

                total += 1;
                aggregate_score += score;
                if score == 1.0 {
                    total_correct += 1;
                } else if score > 0.0 {
                    total_partially_correct += 1;
                }
            } else if let Err(QuizError::ReadlineInterrupted) = result {
                break;
            } else if let Err(e) = result {
                return Err(e);
            }
        }

        let total_incorrect = total - total_correct - total_partially_correct;
        let score = (aggregate_score / (total as f64)) * 100.0;
        Ok(QuizResult {
            time_taken: chrono::Utc::now(),
            total,
            total_correct,
            total_partially_correct,
            total_incorrect,
            score,
            per_question: results,
        })
    }

    /// Return the questions filtered by the given command-line options (e.g., `--tag`
    /// and `--exclude`).
    fn filter_questions(&self, options: &QuizFilterOptions) -> Vec<&Question> {
        let mut candidates = Vec::new();
        for question in self.questions.iter() {
            if filter_question(question, options) {
                candidates.push(question);
            }
        }
        candidates
    }

    /// Choose a set of questions, filtered by the command-line options.
    fn choose_questions(&self, options: &QuizTakeOptions) -> Vec<&Question> {
        let mut candidates = self.filter_questions(&options.filter_opts);

        // --best and --worst can only be applied to questions with at least one
        // scored response, so we remove questions with no scored responses here.
        if options.best.is_some() || options.worst.is_some() {
            let mut i = 0;
            while i < candidates.len() {
                if aggregate_results(&candidates[i].prior_results).is_none() {
                    candidates.remove(i);
                } else {
                    i += 1;
                }
            }
        }

        if let Some(best) = options.best {
            candidates.sort_by(cmp_questions_best);
            candidates.truncate(best);
        } else if let Some(worst) = options.worst {
            candidates.sort_by(cmp_questions_worst);
            candidates.truncate(worst);
        }

        if let Some(most) = options.most {
            candidates.sort_by(cmp_questions_most);
            candidates.truncate(most);
        } else if let Some(least) = options.least {
            candidates.sort_by(cmp_questions_least);
            candidates.truncate(least);
        }

        if !options.in_order {
            let mut rng = thread_rng();
            candidates.shuffle(&mut rng);
        }

        // Important that this operation comes after the --most and --least flags have
        // been applied, e.g. if --most 50 -n 10 we want to choose 10 questions among
        // the 50 most asked, not the most asked among 10 random questions.
        //
        // Also important that this occurs after shuffling.
        if let Some(num_to_ask) = options.num_to_ask {
            candidates.truncate(num_to_ask);
        }

        candidates
    }
}


/// Return `true` if `q` satisfies the constraints in `options`.
fn filter_question(q: &Question, options: &QuizFilterOptions) -> bool {
    // Either no tags were specified, or `q` has all the specified tags.
    (options.tags.len() == 0 || options.tags.iter().all(|tag| q.tags.contains(tag)))
        // `q` must not have any excluded tags.
        && options.exclude.iter().all(|tag| !q.tags.contains(tag))
        // If `--never` flag is present, question must not have been asked before.
        && (!options.never || q.prior_results.len() == 0)
        && filter_question_by_keywords(q, &options.keywords)
}

/// Return `true` if the text of `q` contains all specified keywords.
fn filter_question_by_keywords(q: &Question, keywords: &Vec<String>) -> bool {
    for keyword in keywords.iter() {
        let mut satisfied = false;
        for text in q.text.iter() {
            if text.to_lowercase().contains(keyword.to_lowercase().as_str()) {
                satisfied = true;
                break
            }
        }

        if !satisfied {
            return false;
        }
    }
    true
}


impl Question {
    /// Return a new short-answer question.
    #[allow(dead_code)]
    pub fn new(text: &str, answer: &str) -> Self {
        let answers = vec![Answer { variants: vec![String::from(answer)] }];
        Question {
            kind: QuestionKind::ShortAnswer, text: vec![String::from(text)],
            tags: Vec::new(), answer_list: answers, candidates: Vec::new(),
            prior_results: Vec::new(), explanations: Vec::new(),
        }
    }

    /// Ask the question, get an answer, and return a `QuestionResult` object. If Ctrl+C
    /// is pressed, return an error.
    ///
    /// The `num` argument is the question number in the quiz, which is printed before
    /// the text of the question.
    pub fn ask(&self, num: usize) -> Result<QuestionResult, QuizError> {
        let mut rng = thread_rng();
        let text = self.text.choose(&mut rng).unwrap();
        let prefix = format!("  ({}) ", num);
        prettyprint_colored(
            &text, Some(&prefix), Some(Color::White), Some(Color::Cyan)
        )?;
        my_print!("\n")?;

        match self.kind {
            QuestionKind::ShortAnswer | QuestionKind::Flashcard => {
                self.ask_short_answer()
            },
            QuestionKind::ListAnswer => {
                self.ask_list_answer()
            },
            QuestionKind::OrderedListAnswer => {
                self.ask_ordered_list_answer()
            }
            QuestionKind::MultipleChoice => {
                self.ask_multiple_choice()
            },
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ShortAnswer` or
    /// `Flashcard`.
    fn ask_short_answer(&self) -> Result<QuestionResult, QuizError> {
        let guess = prompt("> ")?;
        let result = guess.is_some() && self.check_any(guess.as_ref().unwrap());

        if result {
            self.correct()?;
        } else {
            let guess_option = guess.as_ref().map(|s| s.as_str());
            self.incorrect(Some(&self.answer_list[0].variants[0]), guess_option)?;
        }

        let score = if result { 1.0 } else { 0.0 };

        if let Some(guess) = guess {
            Ok(self.result(Some(guess), score))
        } else {
            Ok(self.result(None, score))
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ListAnswer`.
    fn ask_list_answer(&self) -> Result<QuestionResult, QuizError> {
        let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
        for _ in 0..self.answer_list.len() {
            satisfied.push(false);
        }

        let mut count = 0;
        let mut responses = Vec::new();
        while count < self.answer_list.len() {
            if let Some(guess) = prompt("> ")? {
                responses.push(guess.clone());

                let index = self.check_one(&guess);
                if index == self.answer_list.len() {
                    self.incorrect(None, Some(&guess))?;
                    count += 1;
                } else if satisfied[index] {
                    my_println!("{}", "You already said that.".white())?;
                } else {
                    satisfied[index] = true;
                    self.correct()?;
                    count += 1;
                }
            } else {
                self.incorrect(None, None)?;
                break;
            }
        }

        let ncorrect = satisfied.iter().filter(|x| **x).count();
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if ncorrect < self.answer_list.len() {
            my_println!("{}", "\nYou missed:".white())?;
            for (i, correct) in satisfied.iter().enumerate() {
                if !correct {
                    my_println!("  {}", self.answer_list[i].variants[0])?;
                }
            }
            my_println!(
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            )?;
        }

        Ok(self.result_with_list(responses, score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `OrderedListAnswer`.
    fn ask_ordered_list_answer(&self) -> Result<QuestionResult, QuizError> {
        let mut ncorrect = 0;
        let mut responses = Vec::new();
        for answer in self.answer_list.iter() {
            if let Some(guess) = prompt("> ")? {
                responses.push(guess.clone());

                if answer.check(&guess) {
                    self.correct()?;
                    ncorrect += 1;
                } else {
                    self.incorrect(Some(&answer.variants[0]), Some(&guess))?;
                }
            } else {
                self.incorrect(Some(&answer.variants[0]), None)?;
                break;
            }
        }
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if score < 1.0 {
            my_println!(
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            )?;
        }

        Ok(self.result_with_list(responses, score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `MultipleChoice`.
    fn ask_multiple_choice(&self) -> Result<QuestionResult, QuizError> {
        let mut candidates = self.candidates.clone();

        let mut rng = thread_rng();
        // Shuffle once so that we don't always pick the first three candidates listed.
        candidates.shuffle(&mut rng);
        candidates.truncate(3);

        let answer = self.answer_list[0].variants.choose(&mut rng).unwrap();
        candidates.push(answer.clone());
        // Shuffle again so that the position of the correct answer is random.
        candidates.shuffle(&mut rng);

        for (i, candidate) in "abcd".chars().zip(candidates.iter()) {
            let prefix = format!("     ({}) ", i);
            prettyprint(candidate, Some(&prefix))?;
        }

        my_print!("\n")?;
        loop {
            if let Some(guess) = prompt("Enter a letter: ")? {
                if guess.len() != 1 {
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    let guess = &candidates[(index - 97) as usize];
                    if self.check_any(guess) {
                        self.correct()?;
                        return Ok(self.result(Some(answer.clone()), 1.0));
                    } else {
                        self.incorrect(Some(&answer), Some(guess))?;
                        return Ok(self.result(Some(answer.clone()), 0.0));
                    }
                } else {
                    continue;
                }
            } else {
                self.incorrect(Some(&answer), None)?;
                return Ok(self.result(Some(answer.clone()), 0.0));
            }
        }
    }

    /// Construct a `QuestionResult` object.
    fn result(&self, response: Option<String>, score: f64) -> QuestionResult {
        QuestionResult {
            text: self.text[0].clone(),
            score,
            response,
            response_list: None,
            time_asked: chrono::Utc::now(),
        }
    }

    /// Construct a `QuestionResult` object with a list of responses.
    fn result_with_list(&self, responses: Vec<String>, score: f64) -> QuestionResult {
        QuestionResult {
            text: self.text[0].clone(),
            score,
            response: None,
            response_list: Some(responses),
            time_asked: chrono::Utc::now(),
        }
    }

    /// Print a message for correct answers.
    fn correct(&self) -> Result<(), QuizError> {
        my_println!("{}", "Correct!".green())
    }

    /// Print a message for an incorrect answer, indicating that `answer` was the
    /// correct answer.
    fn incorrect(
        &self, answer: Option<&str>, guess: Option<&str>
    ) -> Result<(), QuizError> {
        let explanation = if let Some(guess) = guess {
            if let Some(explanation) = self.get_explanation(&guess) {
                format!(" {}", explanation)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if let Some(answer) = answer {
            let message = format!(
                "{} The correct answer was {}.{}",
                "Incorrect.".red(),
                answer.green(),
                explanation
            );
            prettyprint(&message, None)?;
        } else {
            prettyprint(&format!("{}{}", "Incorrect.".red(), &explanation), None)?;
        }
        Ok(())
    }

    fn get_explanation(&self, guess: &str) -> Option<String> {
        let guess = guess.to_lowercase();
        for (variants, explanation) in self.explanations.iter() {
            if variants.contains(&guess) {
                return Some(explanation.clone());
            }
        }
        None
    }

    /// Return `true` if `guess` matches any of the answers in `self.answer_list`.
    fn check_any(&self, guess: &str) -> bool {
        for answer in self.answer_list.iter() {
            if answer.check(guess) {
                return true;
            }
        }
        false
    }

    /// Return the index of the first answer in `self.answer_list` that `guess`
    /// matches, or `self.answer_list.len()` if `guess` satisfies none.
    fn check_one(&self, guess: &str) -> usize {
        for (i, answer) in self.answer_list.iter().enumerate() {
            if answer.check(guess) {
                return i;
            }
        }
        self.answer_list.len()
    }

    /// Flip flashcards. Does nothing if `self.kind` is not `Flashcard`.
    fn flip(&mut self) {
        if self.kind == QuestionKind::Flashcard {
            let mut rng = thread_rng();
            self.answer_list.shuffle(&mut rng);

            let side1 = self.text.remove(0);
            let side2 = self.answer_list.remove(0).variants.remove(0);

            self.text = vec![side2];
            self.answer_list = vec![Answer { variants: vec![side1] } ];
        }
    }
}


impl Answer {
    /// Return `true` if the given string is equivalent to the Answer object.
    fn check(&self, guess: &str) -> bool {
        for variant in self.variants.iter() {
            if variant.to_lowercase() == guess.to_lowercase() {
                return true;
            }
        }
        false
    }
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
            tags: Vec::new(), exclude: Vec::new(), never: false, keywords: Vec::new(),
        }
    }
}


/// Display a prompt and read a line from standard input continually until the user
/// enters a line with at least one non-whitespace character. If the user presses Ctrl+D
/// then `Ok(None)` is returned. If the user pressed Ctrl+C then `Err(())` is returned.
/// Otherwise, `Ok(Some(line))` is returned where `line` is the last line of input the
/// user entered without leading and trailing whitespace.
fn prompt(message: &str) -> Result<Option<String>, QuizError> {
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


/// Return `true` if the quiz directory is a git repository.
fn is_git_repo() -> bool {
    let mut dirpath = get_quiz_dir_path();
    dirpath.push(".git");
    dirpath.exists()
}


fn git(args: &[&str]) -> Result<(), QuizError> {
    let dir = get_quiz_dir_path();
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .spawn()
        .or(Err(QuizError::CannotRunGit))?;
    child.wait().map_err(QuizError::Io)?;
    Ok(())
}


/// Print `message` to standard output, breaking lines according to the current width
/// of the terminal. If `prefix` is not `None`, then prepend it to the first line and
/// indent all subsequent lines by its length.
fn prettyprint(message: &str, prefix: Option<&str>) -> Result<(), QuizError> {
    prettyprint_colored(message, prefix, None, None)
}


fn prettyprint_colored(
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


/// Prompt the user with a yes-no question and return `true` if they enter yes.
fn yesno(message: &str) -> bool {
    match prompt(message) {
        Ok(Some(response)) => {
            response.trim_start().to_lowercase().starts_with("y")
        },
        _ => false
    }
}


/// Parse command-line arguments.
pub fn parse_options() -> QuizOptions {
    let options = QuizOptions::from_args();

    if let QuizOptions::Results(options) = &options {
        let s = &options.sort;
        if s != "most" && s != "least" && s != "best" && s != "worst" {
            eprintln!("{}: unknown value `{}` for --sort.", "Error".red(), s);
            ::std::process::exit(2);
        }
    }

    options
}


/// Print a list of tags.
fn list_tags(quiz: &Quiz) -> Result<(), QuizError> {
    // Count how many times each tag has been used.
    let mut tags = HashMap::<&str, u32>::new();
    for question in quiz.questions.iter() {
        for tag in question.tags.iter() {
            if let Some(n) = tags.get(tag.as_str()) {
                tags.insert(tag.as_str(), n+1);
            } else {
                tags.insert(tag.as_str(), 1);
            }
        }
    }

    if tags.len() == 0 {
        my_println!("No questions have been assigned tags.")?;
    } else {
        my_println!("Available tags:")?;

        let mut tags_in_order: Vec<(&str, u32)> = tags.into_iter().collect();
        tags_in_order.sort();
        for (tag, count) in tags_in_order.iter() {
            my_println!("  {} ({})", tag, count)?;
        }
    }
    Ok(())
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
fn save_results(name: &str, results: &QuizResult) -> Result<(), QuizError> {
    // Load old data, if it exists.
    let path = get_results_path(name);
    let data = fs::read_to_string(&path);
    let mut hash: HashMap<String, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .map_err(QuizError::Json)?
        },
        Err(_) => {
            HashMap::new()
        }
    };

    // Store the results as a map from the text of the questions to a list of individual
    // time-stamped results.
    for result in results.per_question.iter() {
        if !hash.contains_key(&result.text) {
            hash.insert(result.text.to_string(), Vec::new());
        }
        hash.get_mut(&result.text).unwrap().push(result.clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .map_err(QuizError::Json)?;
    fs::write(&path, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(path.clone())))?;
    Ok(())
}


/// Load a `Quiz` object given its name.
fn load_quiz(name: &str) -> Result<Quiz, QuizError> {
    let path = get_quiz_path(name);
    let results_path = get_results_path(name);
    load_quiz_from_file(name, &path, &results_path)
}


/// Load a `Quiz` object from a file. The `name` argument is used only for nice error
/// messages.
fn load_quiz_from_file(
    name: &str, path: &PathBuf, results_path: &PathBuf
) -> Result<Quiz, QuizError> {
    let mut quiz = parser::parse(&path)?;

    // Attach previous results to the `Question` objects.
    let old_results = load_results_from_file(results_path)?;
    for question in quiz.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.text[0]) {
            question.prior_results = results.clone();
        }
    }

    Ok(quiz)
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


fn load_results(name: &str) -> Result<StoredResults, QuizError> {
    let path = get_results_path(name);
    load_results_from_file(&path)
}


fn load_results_from_file(path: &PathBuf) -> Result<StoredResults, QuizError> {
    match fs::read_to_string(path) {
        Ok(data) => {
            serde_json::from_str(&data).map_err(QuizError::Json)
        },
        Err(_) => {
            Ok(HashMap::new())
        }
    }
}


/// Return the percentage of correct responses in the vector of results. `None` is
/// returned when the vector is empty.
fn aggregate_results(results: &Vec<QuestionResult>) -> Option<f64> {
    let mut sum = 0.0;
    let mut graded_count = 0;
    for result in results.iter() {
        sum += result.score;
        graded_count += 1;
    }

    if graded_count > 0 {
        Some(100.0 * (sum / (graded_count as f64)))
    } else {
        None
    }
}


fn cmp_string_ignore_dot(a: &String, b: &String) -> Ordering {
    fn cmp_helper(a: &str, b: &str) -> Ordering {
        if a.starts_with(".") {
            cmp_helper(&a[1..], b)
        } else if b.starts_with(".") {
            cmp_helper(a, &b[1..])
        } else {
            a.cmp(b)
        }
    }

    cmp_helper(a, b)
}


/// An alias for a commonly-used typed in comparison functions.
type CmpQuestionResult = (f64, usize, String);


/// Comparison function that sorts an array of question results such that the best
/// results come first.
fn cmp_results_best(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    if a.0 < b.0 {
        return Ordering::Greater;
    } else if a.0 > b.0 {
        return Ordering::Less;
    } else {
        return cmp_results_most(a, b);
    }
}


/// Comparison function that sorts an array of question results such that the worst
/// results come first.
fn cmp_results_worst(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    return cmp_results_best(a, b).reverse();
}


/// Comparison function that sorts an array of question results such that the results
/// with the most attempts come first.
fn cmp_results_most(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    if a.1 < b.1 {
        return Ordering::Greater;
    } else if a.1 > b.1 {
        return Ordering::Less;
    } else {
        return Ordering::Equal;
    }
}


/// Comparison function that sorts an array of question results such that the results
/// with the least attempts come first.
fn cmp_results_least(a: &CmpQuestionResult, b: &CmpQuestionResult) -> Ordering {
    return cmp_results_most(a, b).reverse();
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the highest previous scores come first.
fn cmp_questions_best(a: &&Question, b: &&Question) -> Ordering {
    let a_score = aggregate_results(&a.prior_results).unwrap_or(0.0);
    let b_score = aggregate_results(&b.prior_results).unwrap_or(0.0);

    if a_score > b_score {
        Ordering::Less
    } else if a_score < b_score {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the lowest previous scores come first.
fn cmp_questions_worst(a: &&Question, b: &&Question) -> Ordering {
    return cmp_questions_best(a, b).reverse();
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the most responses come first.
fn cmp_questions_most(a: &&Question, b: &&Question) -> Ordering {
    let a_results = a.prior_results.len();
    let b_results = b.prior_results.len();

    if a_results > b_results {
        Ordering::Less
    } else if a_results < b_results {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the least responses come first.
fn cmp_questions_least(a: &&Question, b: &&Question) -> Ordering {
    return cmp_questions_most(a, b).reverse();
}


/// Return the path to the file where results are stored for the given quiz.
fn get_results_path(quiz_name: &str) -> PathBuf {
    let mut dirpath = get_app_dir_path();
    dirpath.push("results");
    dirpath.push(format!("{}_results.json", quiz_name));
    dirpath
}


/// Return the path to the file where the given quiz is stored.
fn get_quiz_path(quiz_name: &str) -> PathBuf {
    let mut dirpath = get_quiz_dir_path();
    dirpath.push(quiz_name);
    dirpath
}


/// Return the path to the application directory.
fn get_app_dir_path() -> PathBuf {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    dirpath
}


/// Return the path to the quiz directory.
fn get_quiz_dir_path() -> PathBuf {
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");
    dirpath
}


/// Return the path to the application directory, creating it and all necessary
/// subdirectories if they don't exist.
pub fn require_app_dir_path() -> Result<PathBuf, QuizError> {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    dirpath.push("results");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    dirpath.pop();
    dirpath.push("quizzes");
    make_directory(&dirpath).or(Err(QuizError::CannotMakeAppDir))?;

    Ok(dirpath)
}


fn make_directory(path: &PathBuf) -> Result<(), std::io::Error> {
    if !path.as_path().exists() {
        fs::create_dir(path)?;
    }
    Ok(())
}


#[derive(Debug)]
pub enum QuizError {
    /// For when the application directory cannot be created.
    CannotMakeAppDir,
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
            QuizError::CannotMakeAppDir => {
                // String::from is necessary here for Rust's borrow checker for some
                // reason.
                let path = String::from(get_app_dir_path().to_string_lossy());
                write!(f, "unable to create application directory at {}", path)
            },
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


pub fn is_broken_pipe(e: &QuizError) -> bool {
    if let QuizError::Io(e) = e {
        if let io::ErrorKind::BrokenPipe = e.kind() {
            return true;
        }
    }
    false
}


impl error::Error for QuizError {
    fn cause(&self) -> Option<&error::Error> {
        match *self {
            QuizError::Json(ref err) => Some(err),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_filter_by_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(s("geography"));

        let mut options = QuizFilterOptions::new();
        assert!(filter_question(&q, &options));

        options.tags.push(s("geography"));
        assert!(filter_question(&q, &options));

        options.tags.push(s("history"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_excluding_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(s("geography"));

        let mut options = QuizFilterOptions::new();
        options.exclude.push(s("geography"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_keyword() {
        let q = Question::new("What is the capital of China", "Beijing");

        let mut options = QuizFilterOptions::new();
        options.keywords.push(s("china"));
        assert!(filter_question(&q, &options));

        options.keywords.push(s("river"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn checking_answers_works() {
        let ans = Answer {
            variants: vec![s("Barack Obama"), s("Obama")]
        };

        assert!(ans.check("Barack Obama"));
        assert!(ans.check("barack obama"));
        assert!(ans.check("Obama"));
        assert!(ans.check("obama"));
        assert!(!ans.check("Mitt Romney"));
    }

    fn s(mystr: &str) -> String {
        String::from(mystr)
    }
}
