/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: July 2019
 */
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rustyline::error::ReadlineError;
use serde::{Serialize, Deserialize};
use structopt::StructOpt;


/// Represents an entire quiz.
#[derive(Serialize, Deserialize, Debug)]
struct Quiz {
    questions: Vec<Question>,
}


/// Represents a question.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct Question {
    kind: QuestionKind,
    /// The text of the question. It is a vector instead of a string so that multiple
    /// variants of the same question can be stored.
    text: Vec<String>,
    /// Correct answers to the question. When `kind` is equal to `ShortAnswer` or
    /// `MultipleChoice`, this vector should have only one element.
    answer_list: Vec<Answer>,
    /// Candidate answers to the question. This field is only used when `kind` is set to
    /// `MultipleChoice`, in which case the candidates are incorrect answers to the
    /// question.
    #[serde(default)]
    candidates: Vec<String>,
    /// Prior results of answering the question.
    #[serde(default)]
    prior_results: Vec<QuestionResult>,
    /// Optional string identifier.
    id: Option<String>,
    /// If provided, should be the `id` of another `Question` which must be asked before
    /// this one.
    depends: Option<String>,
    /// User-defined tags for the question.
    #[serde(default)]
    tags: Vec<String>,
    /// Incorrect answers may be given specific explanations for why they are not
    /// right.
    #[serde(default)]
    explanations: Vec<(Vec<String>, String)>,
}


/// An enumeration for the `kind` field of `Question` objects.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice, Ungraded,
}


/// Represents an answer.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct Answer {
    /// Each member of the `variants` vector should be an equivalent answer, e.g.
    /// `vec!["Mount Everest", "Everest"]`, not different answers to the same question.
    /// The first element of the vector is taken to be the canonical form of the answer
    /// for display.
    variants: Vec<String>,
}


/// Represents the result of answering a question on a particular occasion.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct QuestionResult {
    #[serde(skip)]
    text: String,
    time_asked: chrono::DateTime<chrono::Utc>,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    response: Option<String>,
    /// Optional, because ungraded questions don't have scores.
    score: Option<f64>,

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
struct QuizResult {
    time_taken: chrono::DateTime<chrono::Utc>,
    total_answered: usize,
    total_correct: usize,
    total_partially_correct: usize,
    total_incorrect: usize,
    total_ungraded: usize,
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
    #[structopt(name = "delete")]
    Delete(QuizDeleteOptions),
    /// List all available quizzes.
    #[structopt(name = "list")]
    List,
}

#[derive(StructOpt)]
pub struct QuizTakeOptions {
    /// Name of the quiz to take.
    #[structopt(default_value = "main")]
    name: String,
    /// Limit the total number of questions.
    #[structopt(short = "n")]
    num_to_ask: Option<usize>,
    /// Choose from the `n` questions with the highest previous scores.
    #[structopt(long = "best")]
    best: Option<usize>,
    /// Choose from the `n` questions with the lowest previous scores.
    #[structopt(long = "worst")]
    worst: Option<usize>,
    /// Choose from the `n` questions with the most previous attempts.
    #[structopt(long = "most")]
    most: Option<usize>,
    /// Choose from the `n` questions with the least previous attempts.
    #[structopt(long = "least")]
    least: Option<usize>,
    /// Save results without prompting.
    #[structopt(long = "save")]
    save: bool,
    /// Do not emit colorized output.
    #[structopt(long = "no-color")]
    no_color: bool,
    /// Ask the questions in the order they appear in the quiz file.
    #[structopt(long = "in-order")]
    in_order: bool,
    #[structopt(flatten)]
    filter_opts: QuizFilterOptions,
}

#[derive(StructOpt)]
pub struct QuizCountOptions {
    /// Name of the quiz to count.
    #[structopt(default_value = "main")]
    name: String,
    /// List tags instead of counting questions.
    #[structopt(long = "list-tags")]
    list_tags: bool,
    #[structopt(flatten)]
    filter_opts: QuizFilterOptions,
}

/// These filtering options are shared between the `take` and `count` subcommands.
#[derive(StructOpt)]
pub struct QuizFilterOptions {
    /// Only include questions with the given tag.
    #[structopt(long = "tag")]
    tags: Vec<String>,
    /// Exclude questions with the given tag.
    #[structopt(long = "exclude")]
    exclude: Vec<String>,
    /// Only include questions that have never been asked before.
    #[structopt(long = "never")]
    never: bool,
    /// Filter by keyword.
    #[structopt(short = "k", long = "keyword")]
    keywords: Vec<String>,
}

#[derive(StructOpt)]
pub struct QuizEditOptions {
    /// The name of the quiz to edit.
    #[structopt(default_value = "main")]
    name: String,
}

#[derive(StructOpt)]
pub struct QuizDeleteOptions {
    /// The name of the quiz to delete.
    #[structopt(default_value = "main")]
    name: String,
    /// Delete without prompting for confirmation.
    #[structopt(short = "f", long = "force")]
    force: bool,
}

#[derive(StructOpt)]
pub struct QuizResultsOptions {
    /// The name of the quiz for which to fetch the results.
    #[structopt(default_value = "main")]
    name: String,
    /// One of 'best', 'worst', 'most' or 'least'. Defaults to 'best'.
    #[structopt(short = "s", long = "sort", default_value = "best")]
    sort: String,
    /// Only show the first `n` results.
    #[structopt(short = "n")]
    num_to_show: Option<usize>,
}


// One main function for each subcommand.


/// The main function for the `take` subcommand.
pub fn main_take<W: io::Write, R: MyReadline>(
    writer: &mut W, reader: &mut R, options: QuizTakeOptions
) -> Result<(), QuizError> {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = load_quiz(&options.name)?;
    let results = quiz.take(writer, reader, &options)?;
    output_results(writer, &results)?;

    let total_graded = results.total_answered - results.total_ungraded;
    if total_graded > 0 && (options.save || yesno(reader, "\nSave results? ")) {
        save_results(&options.name, &results)?;
    }
    Ok(())
}


fn output_results<W: io::Write>(
    writer: &mut W, results: &QuizResult
) -> Result<(), QuizError> {
    let total_graded = results.total_answered - results.total_ungraded;
    if total_graded > 0 {
        let score_as_str = format!("{:.1}%", results.score);

        my_write!(writer, "\n\n")?;
        my_write!(writer, "{}", "Score: ".white())?;
        my_write!(writer, "{}", score_as_str.cyan())?;
        my_write!(writer, "{}", " out of ".white())?;
        my_write!(writer, "{}", format!("{}", results.total_answered).cyan())?;
        if results.total_answered == 1 {
            my_writeln!(writer, "{}", " question".white())?;
        } else {
            my_writeln!(writer, "{}", " questions".white())?;
        }
        my_write!(writer, "  {}", format!("{}", results.total_correct).bright_green())?;
        my_write!(writer, "{}\n", " correct".white())?;
        my_write!(writer, "  {}", format!("{}", results.total_partially_correct).green())?;
        my_write!(writer, "{}\n", " partially correct".white())?;
        my_write!(writer, "  {}", format!("{}", results.total_incorrect).red())?;
        my_write!(writer, "{}\n", " incorrect".white())?;
        my_write!(writer, "  {}", format!("{}", results.total_ungraded).cyan())?;
        my_write!(writer, "{}\n", " ungraded".white())?;
    } else if results.total_ungraded > 0 {
        my_writeln!(writer, "{}", "\n\nAll questions were ungraded.".white())?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count<W: io::Write>(
    writer: &mut W, options: QuizCountOptions
) -> Result<(), QuizError> {
    let quiz = load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(writer, &quiz)?;
    } else {
        let filtered = quiz.filter_questions(&options.filter_opts);
        my_writeln!(writer, "{}", filtered.len())?;
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results<W: io::Write>(
    writer: &mut W, options: QuizResultsOptions
) -> Result<(), QuizError> {
    let results = load_results(&options.name)?;

    if results.len() == 0 {
        my_writeln!(writer, "No results have been recorded for this quiz.")?;
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
        prettyprint_colored(
            writer, &question, Some(&first_prefix), None, Some(Color::Cyan)
        )?;
    }

    Ok(())
}


pub fn main_edit(options: QuizEditOptions) -> Result<(), QuizError> {
    require_app_dir_path()?;

    let path = get_quiz_path(&options.name);
    let editor = ::std::env::var("EDITOR").unwrap_or(String::from("vim"));
    let mut child = Command::new(editor).arg(path).spawn()
        .or(Err(QuizError::CannotOpenEditor))?;
    child.wait()
        .or(Err(QuizError::CannotOpenEditor))?;
    Ok(())
}


pub fn main_delete<R: MyReadline>(
    reader: &mut R, options: QuizDeleteOptions
) -> Result<(), QuizError> {
    require_app_dir_path()?;

    let path = get_quiz_path(&options.name);
    if path.exists() {
        let yesno_prompt = "Are you sure you want to delete the quiz? ";
        if options.force || yesno(reader, yesno_prompt) {
            fs::remove_file(&path).map_err(QuizError::Io)?;
        }
        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.clone()))
    }
}


pub fn main_list<W: io::Write>(writer: &mut W) -> Result<(), QuizError> {
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");

    if let Ok(iter) = dirpath.read_dir() {
        let mut found_any = false;
        for entry in iter {
            if let Ok(entry) = entry {
                if let Some(stem) = entry.path().file_stem() {
                    if let Some(stem) = stem.to_str() {
                        if !found_any {
                            my_writeln!(writer, "Available quizzes:")?;
                            found_any = true;
                        }
                        my_writeln!(writer, "  {}", stem)?;
                    }
                }
            }
        }

        if !found_any {
            my_writeln!(writer, "No quizzes found.")?;
        }
    } else {
        my_writeln!(writer, "No quizzes found.")?;
    }
    Ok(())
}


impl Quiz {
    /// Take the quiz and return pairs of questions and results.
    fn take<W: io::Write, R: MyReadline>(
        &mut self, writer: &mut W, reader: &mut R, options: &QuizTakeOptions
    ) -> Result<QuizResult, QuizError> {
        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total_partially_correct = 0;
        let mut total_ungraded = 0;
        let mut total = 0;
        let mut aggregate_score = 0.0;

        let questions = self.choose_questions(&options);
        if questions.len() == 0 {
            return Err(QuizError::EmptyQuiz);
        }

        for (i, question) in questions.iter().enumerate() {
            my_write!(writer, "\n")?;
            let result = question.ask(writer, reader, i+1);
            if let Ok(result) = result {
                let score_option = result.score;
                results.push(result);

                if let Some(score) = score_option {
                    total += 1;
                    aggregate_score += score;
                    if score == 1.0 {
                        total_correct += 1;
                    } else if score > 0.0 {
                        total_partially_correct += 1;
                    }
                } else {
                    total_ungraded += 1;
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
            total_answered: total + total_ungraded,
            total_correct,
            total_partially_correct,
            total_incorrect,
            total_ungraded,
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

        // Important that this operation comes after the --most and --least flags have
        // been applied, e.g. if --most 50 -n 10 we want to choose 10 questions among
        // the 50 most asked, not the most asked among 10 random questions.
        if let Some(num_to_ask) = options.num_to_ask {
            candidates.truncate(num_to_ask);
        }

        if !options.in_order {
            let mut rng = thread_rng();
            candidates.shuffle(&mut rng);
        }

        // Respect basic dependence relations.
        for i in 0..candidates.len() {
            for j in (i+1)..candidates.len() {
                if let Some(id) = &candidates[j].id {
                    if let Some(depends) = &candidates[i].depends {
                        if id == depends {
                            candidates.swap(i, j);
                        }
                    }
                }
            }
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
    fn new(text: &str, answer: &str) -> Self {
        let answers = vec![Answer { variants: vec![String::from(answer)] }];
        Question {
            kind: QuestionKind::ShortAnswer, text: vec![String::from(text)],
            tags: Vec::new(), answer_list: answers, candidates: Vec::new(),
            prior_results: Vec::new(), id: None, depends: None,
            explanations: Vec::new(),
        }
    }

    /// Ask the question, get an answer, and return a `QuestionResult` object. If Ctrl+C
    /// is pressed, return an error.
    ///
    /// The `num` argument is the question number in the quiz, which is printed before
    /// the text of the question.
    fn ask<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R, num: usize
    ) -> Result<QuestionResult, QuizError> {
        let mut rng = thread_rng();
        let text = self.text.choose(&mut rng).unwrap();
        let prefix = format!("  ({}) ", num);
        prettyprint_colored(
            writer, &text, Some(&prefix), Some(Color::White), Some(Color::Cyan)
        )?;
        my_write!(writer, "\n")?;

        match self.kind {
            QuestionKind::ShortAnswer => {
                self.ask_short_answer(writer, reader)
            },
            QuestionKind::ListAnswer => {
                self.ask_list_answer(writer, reader)
            },
            QuestionKind::OrderedListAnswer => {
                self.ask_ordered_list_answer(writer, reader)
            }
            QuestionKind::MultipleChoice => {
                self.ask_multiple_choice(writer, reader)
            },
            QuestionKind::Ungraded => {
                self.ask_ungraded(writer, reader)
            }
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ShortAnswer`.
    fn ask_short_answer<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R
    ) -> Result<QuestionResult, QuizError> {
        let guess = prompt(reader, "> ")?;
        let result = guess.is_some() && self.check_any(guess.as_ref().unwrap());

        if result {
            self.correct(writer)?;
        } else {
            let guess_option = guess.as_ref().map(|s| s.as_str());
            self.incorrect(
                writer, Some(&self.answer_list[0].variants[0]), guess_option
            )?;
        }

        let score = if result { 1.0 } else { 0.0 };

        if let Some(guess) = guess {
            Ok(self.result(Some(guess.to_lowercase()), Some(score)))
        } else {
            Ok(self.result(None, Some(score)))
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ListAnswer`.
    fn ask_list_answer<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R
    ) -> Result<QuestionResult, QuizError> {
        let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
        for _ in 0..self.answer_list.len() {
            satisfied.push(false);
        }

        let mut count = 0;
        while count < self.answer_list.len() {
            if let Some(guess) = prompt(reader, "> ")? {
                let index = self.check_one(&guess);
                if index == self.answer_list.len() {
                    self.incorrect(writer, None, Some(&guess))?;
                    count += 1;
                } else if satisfied[index] {
                    my_writeln!(writer, "{}", "You already said that.".white())?;
                } else {
                    satisfied[index] = true;
                    self.correct(writer)?;
                    count += 1;
                }
            } else {
                self.incorrect(writer, None, None)?;
                break;
            }
        }

        let ncorrect = satisfied.iter().filter(|x| **x).count();
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if ncorrect < self.answer_list.len() {
            my_writeln!(writer, "{}", "\nYou missed:".white())?;
            for (i, correct) in satisfied.iter().enumerate() {
                if !correct {
                    my_writeln!(writer, "  {}", self.answer_list[i].variants[0])?;
                }
            }
            my_writeln!(
                writer,
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            )?;
        }
        Ok(self.result(None, Some(score)))
    }

    /// Implementation of `ask` assuming that `self.kind` is `OrderedListAnswer`.
    fn ask_ordered_list_answer<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R
    ) -> Result<QuestionResult, QuizError> {
        let mut ncorrect = 0;
        for answer in self.answer_list.iter() {
            if let Some(guess) = prompt(reader, "> ")? {
                if answer.check(&guess) {
                    self.correct(writer)?;
                    ncorrect += 1;
                } else {
                    self.incorrect(writer, Some(&answer.variants[0]), Some(&guess))?;
                }
            } else {
                self.incorrect(writer, Some(&answer.variants[0]), None)?;
                break;
            }
        }
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if score < 1.0 {
            my_writeln!(
                writer,
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            )?;
        }
        Ok(self.result(None, Some(score)))
    }

    /// Implementation of `ask` assuming that `self.kind` is `MultipleChoice`.
    fn ask_multiple_choice<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R
    ) -> Result<QuestionResult, QuizError> {
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
            prettyprint(writer, candidate, Some(&prefix))?;
        }

        my_write!(writer, "\n")?;
        loop {
            if let Some(guess) = prompt(reader, "Enter a letter: ")? {
                if guess.len() != 1 {
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    let guess = &candidates[(index - 97) as usize];
                    if self.check_any(guess) {
                        self.correct(writer)?;
                        return Ok(self.result(Some(answer.clone()), Some(1.0)));
                    } else {
                        self.incorrect(writer, Some(&answer), Some(guess))?;
                        return Ok(self.result(Some(answer.clone()), Some(0.0)));
                    }
                } else {
                    continue;
                }
            } else {
                self.incorrect(writer, Some(&answer), None)?;
                return Ok(self.result(Some(answer.clone()), Some(0.0)));
            }
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `Ungraded`.
    fn ask_ungraded<W: io::Write, R: MyReadline>(
        &self, writer: &mut W, reader: &mut R
    ) -> Result<QuestionResult, QuizError> {
        let response = prompt(reader, "> ")?;
        my_writeln!(writer, "\n{}", "Sample correct answer:\n".white())?;
        prettyprint(writer, &self.answer_list[0].variants[0], Some("  "))?;
        Ok(self.result(response, None))
    }

    /// Construct a `QuestionResult` object.
    fn result(&self, response: Option<String>, score: Option<f64>) -> QuestionResult {
        QuestionResult {
            text: self.text[0].clone(),
            score,
            response,
            time_asked: chrono::Utc::now(),
        }
    }

    /// Print a message for correct answers.
    fn correct<W: io::Write>(&self, writer: &mut W) -> Result<(), QuizError> {
        my_writeln!(writer, "{}", "Correct!".green())
    }

    /// Print a message for an incorrect answer, indicating that `answer` was the
    /// correct answer.
    fn incorrect<W: io::Write>(
        &self, writer: &mut W, answer: Option<&str>, guess: Option<&str>
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
            prettyprint(writer, &message, None)?;
        } else {
            prettyprint(
                writer, &format!("{}{}", "Incorrect.".red(), &explanation), None
            )?;
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
    fn new() -> Self {
        QuizTakeOptions {
            name: String::new(), num_to_ask: None, best: None, worst: None, most: None,
            least: None, save: false, no_color: true, in_order: false,
            filter_opts: QuizFilterOptions::new()
        }
    }
}


impl QuizFilterOptions {
    #[allow(dead_code)]
    fn new() -> Self {
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
fn prompt<R: MyReadline>(reader: &mut R, message: &str) -> Result<Option<String>, QuizError> {
    loop {
        let result = reader.read_line(message);
        match result {
            Ok(response) => {
                let response = response.trim();
                if response.len() > 0 {
                    return Ok(Some(response.to_string()));
                }
            },
            // Return immediately if the user hits Ctrl+D or Ctrl+C.
            Err(QuizError::ReadlineInterrupted) => {
                return Err(QuizError::ReadlineInterrupted);
            },
            Err(QuizError::ReadlineEof) => {
                return Ok(None);
            },
            _ => {}
        }

    }
}


/// Print `message` to standard output, breaking lines according to the current width
/// of the terminal. If `prefix` is not `None`, then prepend it to the first line and
/// indent all subsequent lines by its length.
fn prettyprint<W: io::Write>(
    writer: &mut W, message: &str, prefix: Option<&str>
) -> Result<(), QuizError> {
    prettyprint_colored(writer, message, prefix, None, None)
}


fn prettyprint_colored<W: io::Write>(
    writer: &mut W, message: &str, prefix: Option<&str>, message_color: Option<Color>,
    prefix_color: Option<Color>
) -> Result<(), QuizError> {
    let prefix = prefix.unwrap_or("");
    let width = textwrap::termwidth() - prefix.len();
    let mut lines = textwrap::wrap_iter(message, width);

    if let Some(first_line) = lines.next() {
        let colored_prefix = color_optional(&prefix, prefix_color);
        let colored_line = color_optional(&first_line, message_color);
        my_writeln!(writer, "{}{}", colored_prefix, colored_line)?;
    }

    let indent = " ".repeat(prefix.len());
    for line in lines {
        let colored_line = color_optional(&line, message_color);
        my_writeln!(writer, "{}{}", indent, colored_line)?;
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
fn yesno<R: MyReadline>(reader: &mut R, message: &str) -> bool {
    match prompt(reader, message) {
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
fn list_tags<W: io::Write>(writer: &mut W, quiz: &Quiz) -> Result<(), QuizError> {
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
        my_writeln!(writer, "No questions have been assigned tags.")?;
    } else {
        my_writeln!(writer, "Available tags:")?;

        let mut tags_in_order: Vec<(&str, u32)> = tags.into_iter().collect();
        tags_in_order.sort();
        for (tag, count) in tags_in_order.iter() {
            my_writeln!(writer, "  {} ({})", tag, count)?;
        }
    }
    Ok(())
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
fn save_results(name: &str, results: &QuizResult) -> Result<(), QuizError> {
    require_app_dir_path()?;

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
fn load_quiz_from_file(name: &str, path: &PathBuf, results_path: &PathBuf) -> Result<Quiz, QuizError> {
    let data = fs::read_to_string(path)
        .or(Err(QuizError::QuizNotFound(name.to_string())))?;

    let mut quiz = load_quiz_from_json(&data)?;

    // Attach previous results to the `Question` objects.
    let old_results = load_results_from_file(results_path)?;
    for question in quiz.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.text[0]) {
            question.prior_results = results.clone();
        }
    }

    Ok(quiz)
}


/// Load a `Quiz` object from a string containing JSON data.
fn load_quiz_from_json(data: &str) -> Result<Quiz, QuizError> {
    let mut quiz_as_json: serde_json::Value = serde_json::from_str(&data)
        .map_err(QuizError::Json)?;

    // Expand each JSON object before doing strongly-typed deserialization.
    if let Some(quiz_as_object) = quiz_as_json.as_object_mut() {
        if let Some(questions) = quiz_as_object.get_mut("questions") {
            if let Some(questions_as_array) = questions.as_array_mut() {
                for i in 0..questions_as_array.len() {
                    // Expand each individual question object.
                    if let Some(question) = questions_as_array[i].as_object() {
                        questions_as_array[i] = serde_json::to_value(
                            normalize_question_json(&question)
                        ).unwrap();
                    }
                }
            }
        }
    }

    let ret: Quiz = serde_json::from_value(quiz_as_json)
        .map_err(QuizError::Json)?;
    Ok(ret)
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


type JSONMap = serde_json::Map<String, serde_json::Value>;


/// Given a JSON object in the disk format, return an equivalent JSON object in the
/// format that the deserialization library understands (i.e., a format that is
/// isomorphic to the fields of the `Question` struct).
fn normalize_question_json(question: &JSONMap) -> JSONMap {
    let mut ret = question.clone();

    // The `kind` field defaults to "ShortAnswer".
    if !ret.contains_key("kind") {
        ret.insert(String::from("kind"), serde_json::json!("ShortAnswer"));
    }

    // Convert answer objects from [...] to { "variants": [...] }.
    if let Some(answer_list) = question.get("answer_list") {
        if let Some(answers_as_array) = answer_list.as_array() {
            ret.remove("answer_list");
            let mut new_answers = Vec::new();
            for i in 0..answers_as_array.len() {
                if answers_as_array[i].is_array() {
                    new_answers.push(
                        serde_json::json!({"variants": answers_as_array[i].clone()})
                    );
                } else if answers_as_array[i].is_string() {
                    new_answers.push(
                        serde_json::json!({"variants": [answers_as_array[i].clone()]})
                    );
                } else {
                    // If not an array, don't touch it.
                    new_answers.push(answers_as_array[i].clone());
                }
            }

            // Replace the old answer_list array with the newly constructed one.
            ret.insert(
                String::from("answer_list"), serde_json::to_value(new_answers).unwrap()
            );
        }
    }

    // Text fields of the form `[text]` may be abbreviated as just `text`.
    if let Some(text) = ret.get("text") {
        if text.is_string() {
            ret.insert(String::from("text"), serde_json::json!([text]));
        }
    }

    // Multiple-choice and short answer questions may use an `answer` field with a
    // single value rather than an `answer_list` field with an array of values.
    if !ret.contains_key("answer_list") {
        if let Some(answer) = ret.get("answer") {
            if answer.is_array() {
                // If array, make {"variants": answer}
                ret.insert(
                    String::from("answer_list"),
                    serde_json::json!([{"variants": answer.clone()}])
                );
            } else {
                // If not array, make {"variants": [answer]}
                ret.insert(
                    String::from("answer_list"),
                    serde_json::json!([{"variants": [answer.clone()]}])
                );
            }
            ret.remove("answer");
        }
    }

    ret
}


/// Return the percentage of correct responses in the vector of results. `None` is
/// returned when the vector is empty or none of the results were graded (i.e., for
/// ungraded questions).
fn aggregate_results(results: &Vec<QuestionResult>) -> Option<f64> {
    let mut sum = 0.0;
    let mut graded_count = 0;
    for result in results.iter() {
        if let Some(score) = result.score {
            sum += score;
            graded_count += 1;
        }
    }

    if graded_count > 0 {
        Some(100.0 * (sum / (graded_count as f64)))
    } else {
        None
    }
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
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");
    dirpath.push(format!("{}.json", quiz_name));
    dirpath
}


/// Return the path to the application directory.
fn get_app_dir_path() -> PathBuf {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    dirpath
}


/// Return the path to the application directory, creating it and all necessary
/// subdirectories if they don't exist.
fn require_app_dir_path() -> Result<PathBuf, QuizError> {
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
    CannotWriteToFile(PathBuf),
    Io(io::Error),
    ReadlineInterrupted,
    ReadlineEof,
    ReadlineOther,
    EmptyQuiz,
}


impl fmt::Display for QuizError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            QuizError::CannotMakeAppDir => {
                if let Some(path) = get_app_dir_path().to_str() {
                    write!(f, "unable to create application directory at {}", path)
                } else {
                    write!(f, "unable to create application directory")
                }
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
            QuizError::CannotWriteToFile(ref path) => {
                if let Some(path) = path.to_str() {
                    write!(f, "cannot write to file '{}'", path)
                } else {
                    write!(
                        f, "cannot write to file and cannot convert file name to UTF-8"
                    )
                }
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
            QuizError::ReadlineEof => {
                Ok(())
            },
            QuizError::ReadlineOther => {
                write!(f, "error while reading input")
            },
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


#[macro_export]
macro_rules! my_writeln {
    ($dst:expr, $($arg:tt)*) => (
        writeln!($dst, $($arg)*).map_err(QuizError::Io)
    );
}


#[macro_export]
macro_rules! my_write {
    ($dst:expr, $($arg:tt)*) => (
        write!($dst, $($arg)*).map_err(QuizError::Io)
    );
}


pub trait MyReadline {
    fn read_line(&mut self, prompt: &str) -> Result<String, QuizError>;
}

impl<H: rustyline::Helper> MyReadline for rustyline::Editor<H> {
    fn read_line(&mut self, prompt: &str) -> Result<String, QuizError> {
        match self.readline(&format!("{}", prompt.white())) {
            Ok(s) => Ok(s),
            Err(ReadlineError::Interrupted) => Err(QuizError::ReadlineInterrupted),
            Err(ReadlineError::Eof) => Err(QuizError::ReadlineEof),
            _ => Err(QuizError::ReadlineOther),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_filter_by_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(String::from("geography"));

        let mut options = QuizFilterOptions::new();
        assert!(filter_question(&q, &options));

        options.tags.push(String::from("geography"));
        assert!(filter_question(&q, &options));

        options.tags.push(String::from("history"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_excluding_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(String::from("geography"));

        let mut options = QuizFilterOptions::new();
        options.exclude.push(String::from("geography"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_keyword() {
        let q = Question::new("What is the capital of China", "Beijing");

        let mut options = QuizFilterOptions::new();
        options.keywords.push(String::from("china"));
        assert!(filter_question(&q, &options));

        options.keywords.push(String::from("river"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn checking_answers_works() {
        let ans = Answer {
            variants: vec![String::from("Barack Obama"), String::from("Obama")]
        };

        assert!(ans.check("Barack Obama"));
        assert!(ans.check("barack obama"));
        assert!(ans.check("Obama"));
        assert!(ans.check("obama"));
        assert!(!ans.check("Mitt Romney"));
    }

    #[test]
    fn can_expand_short_answer_json() {
        let input = r#"
        {
          "questions": [
            {"text": "When did WW2 start?", "answer": "1939"}
          ]
        }
        "#;
        let output = &load_quiz_from_json(&input).unwrap().questions[0];

        let expected_output = Question {
            kind: QuestionKind::ShortAnswer,
            text: vec![String::from("When did WW2 start?")],
            answer_list: vec![
                Answer { variants: vec![String::from("1939")] }
            ],
            candidates: Vec::new(),
            prior_results: Vec::new(),
            tags: Vec::new(),
            id: None,
            depends: None,
            explanations: Vec::new(),
        };

        assert_eq!(*output, expected_output);
    }

    #[test]
    fn can_expand_list_answer_json() {
        let input = r#"
        {
          "questions": [
            {
              "kind": "ListAnswer",
              "text": "List the four countries of the United Kingdom.",
              "answer_list": [
                  "England", "Scotland", ["Northern Ireland", "N. Ireland"], "Wales"
              ]
            }
          ]
        }
        "#;
        let output = &load_quiz_from_json(&input).unwrap().questions[0];

        let expected_output = Question {
            kind: QuestionKind::ListAnswer,
            text: vec![String::from("List the four countries of the United Kingdom.")],
            answer_list: vec![
                Answer { variants: vec![String::from("England")] },
                Answer { variants: vec![String::from("Scotland")] },
                Answer {
                    variants: vec![
                        String::from("Northern Ireland"),
                        String::from("N. Ireland"),
                    ]
                },
                Answer { variants: vec![String::from("Wales")] },
            ],
            candidates: Vec::new(),
            prior_results: Vec::new(),
            tags: Vec::new(),
            id: None,
            depends: None,
            explanations: Vec::new(),
        };

        assert_eq!(*output, expected_output);
    }

    #[test]
    fn can_expand_multiple_choice_json() {
        let input = r#"
        {
          "questions": [
            {
              "kind": "MultipleChoice",
              "text": "What language is spoken in Cambodia?",
              "candidates": ["Thai", "French", "Vietnamese", "Burmese", "Tagalog"],
              "answer": "Khmer"
            }
          ]
        }
        "#;

        let output = &load_quiz_from_json(&input).unwrap().questions[0];
        let expected_output = Question {
            kind: QuestionKind::MultipleChoice,
            text: vec![String::from("What language is spoken in Cambodia?")],
            candidates: vec![
                String::from("Thai"), String::from("French"), String::from("Vietnamese"),
                String::from("Burmese"), String::from("Tagalog")],
            answer_list: vec![Answer { variants: vec![String::from("Khmer")] } ],
            prior_results: Vec::new(),
            tags: Vec::new(),
            id: None,
            depends: None,
            explanations: Vec::new(),
        };

        assert_eq!(*output, expected_output);
    }

    // #[test]
    // fn can_take_quiz() {
    //     let options = QuizTakeOptions::new();

    //     let stringin = String::new();
    //     let stringout = String::new();
    //     main_take(&mut stringin, &mut stringout, options);
    // }
}
