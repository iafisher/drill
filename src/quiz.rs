/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::cmp::Ordering;
use std::error;
use std::fmt;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Deserialize};
use structopt::StructOpt;

use super::iohelper::{prettyprint, prettyprint_colored, prompt};
use super::parser;
// TODO: Don't depend on persistence.
use super::persistence;


/// Represents an entire quiz.
#[derive(Debug)]
pub struct Quiz {
    pub default_kind: Option<String>,
    pub instructions: Option<String>,
    pub questions: Vec<Question>,
}


/// Represents a question.
#[derive(Debug)]
pub struct Question {
    pub kind: QuestionKind,
    pub id: String,
    /// The text of the question. It is a vector instead of a string so that multiple
    /// variants of the same question can be stored.
    pub text: Vec<String>,
    /// Correct answers to the question. When `kind` is equal to `ShortAnswer` or
    /// `MultipleChoice`, this vector should have only one element.
    pub answer_list: Vec<Answer>,
    /// Candidate answers to the question. This field is only used when `kind` is set to
    /// `MultipleChoice`, in which case the candidates are incorrect answers to the
    /// question.
    pub candidates: Vec<String>,
    /// Prior results of answering the question.
    pub prior_results: Vec<QuestionResult>,
    /// User-defined tags for the question.
    pub tags: Vec<String>,
    /// Incorrect answers may be given specific explanations for why they are not
    /// right.
    pub explanations: Vec<(Vec<String>, String)>,

    /// The location where the question is defined.
    pub location: Option<parser::Location>,
}



/// An enumeration for the `kind` field of `Question` objects.
#[derive(Debug, PartialEq, Eq)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice, Flashcard,
}


/// Represents an answer.
#[derive(Debug, Clone)]
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
    pub id: String,
    /// If the question asked was a short answer question, then the user's response goes
    /// in this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// If the question asked was a list question, then the user's responses go in this
    /// field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_list: Option<Vec<String>>,
    pub score: f64,

    // It would be convenient to include a reference to the `Question` object as a field
    // of this struct, but Rust's lifetime makes it more difficult than it's worth.
}


/// Represents the results of taking a quiz on a particular occasion.
#[derive(Debug)]
pub struct QuizResult {
    pub time_finished: chrono::DateTime<chrono::Utc>,
    pub total: usize,
    pub total_correct: usize,
    pub total_partially_correct: usize,
    pub total_incorrect: usize,
    pub score: f64,
    pub per_question: Vec<QuestionResult>,
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
            time_finished: chrono::Utc::now(),
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
    pub fn filter_questions(&self, options: &QuizFilterOptions) -> Vec<&Question> {
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
            kind: QuestionKind::ShortAnswer,
            id: String::from("1"),
            text: vec![String::from(text)],
            tags: Vec::new(),
            answer_list: answers,
            candidates: Vec::new(),
            prior_results: Vec::new(),
            explanations: Vec::new(),
            location: None,
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
            id: self.id.clone(),
            score,
            response,
            response_list: None,
        }
    }

    /// Construct a `QuestionResult` object with a list of responses.
    fn result_with_list(&self, responses: Vec<String>, score: f64) -> QuestionResult {
        QuestionResult {
            id: self.id.clone(),
            score,
            response: None,
            response_list: Some(responses),
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


/// Return the percentage of correct responses in the vector of results. `None` is
/// returned when the vector is empty.
pub fn aggregate_results(results: &Vec<QuestionResult>) -> Option<f64> {
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
                let path = String::from(persistence::get_app_dir_path().to_string_lossy());
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
