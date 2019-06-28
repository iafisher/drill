/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::fs;
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
    /// User-defined tags for the question.
    tags: Vec<String>,
    /// Correct answers to the question. When `kind` is equal to `ShortAnswer` or
    /// `MultipleChoice`, this vector should have only one element.
    answer_list: Vec<Answer>,
    /// Candidate answers to the question. This field is only used when `kind` is set to
    /// `MultipleChoice`, in which case the candidates are incorrect answers to the
    /// question.
    candidates: Vec<String>,
    /// Prior results of answering the question.
    prior_results: Option<Vec<QuestionResult>>,
    /// Optional string identifier.
    id: Option<String>,
    /// If provided, should be the `id` of another `Question` which must be asked before
    /// this one.
    depends: Option<String>,
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
    /// When combined with -n, take the `n` questions with the highest previous scores.
    #[structopt(long = "best")]
    best: bool,
    /// When combined with -n, take the `n` questions with the lowest previous scores.
    #[structopt(long = "worst")]
    worst: bool,
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
    /// Only include the `n` worst results.
    #[structopt(short = "w", long = "worst")]
    worst: Option<usize>,
    /// Only include the `n` best results.
    #[structopt(short = "b", long = "best")]
    best: Option<usize>,
}


// One main function for each subcommand.


/// The main function for the `take` subcommand.
pub fn main_take(options: QuizTakeOptions) -> Result<(), QuizError> {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = load_quiz(&options.name)?;
    let results = quiz.take(&options);
    if results.len() > 0 && (options.save || yesno("\nSave results? ")) {
        save_results(&options.name, &results)?;
    }
    Ok(())
}


/// The main function for the `count` subcommand.
pub fn main_count(options: QuizCountOptions) -> Result<(), QuizError> {
    let quiz = load_quiz(&options.name)?;
    if options.list_tags {
        list_tags(&quiz);
    } else {
        let filtered = quiz.filter_questions(&options.filter_opts);
        println!("{}", filtered.len());
    }
    Ok(())
}


/// The main function for the `results` subcommand.
pub fn main_results(options: QuizResultsOptions) -> Result<(), QuizError> {
    let results = load_results(&options.name)?;

    if results.len() == 0 {
        println!("No results have been recorded for this quiz.");
        return Ok(());
    }

    let mut aggregated: Vec<(f64, usize, String)> = Vec::new();
    for (key, result) in results.iter() {
        if let Some(score) = aggregate_results(&result) {
            aggregated.push((score, result.len(), key.clone()));
        }
    }

    aggregated.sort_by(cmp_f64_tuple_reversed);

    let best = options.best.unwrap_or(aggregated.len());
    let worst = options.worst.unwrap_or(aggregated.len());
    let iter = aggregated.iter().take(best).skip(aggregated.len() - worst);
    for (score, attempts, question) in iter {
        let first_prefix = format!("{:>5.1}%  of {:>2}   ", score, attempts);
        let width = textwrap::termwidth() - first_prefix.len();
        let mut lines = textwrap::wrap_iter(question, width);

        if let Some(first_line) = lines.next() {
            println!("{}{}", first_prefix.cyan(), first_line);
        }

        let prefix = " ".repeat(first_prefix.len());
        for line in lines {
            println!("{}{}", prefix, line);
        }
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


pub fn main_delete(options: QuizDeleteOptions) -> Result<(), QuizError> {
    require_app_dir_path()?;

    let path = get_quiz_path(&options.name);
    if path.exists() {
        if options.force || yesno("Are you sure you want to delete the quiz? ") {
            if let Err(_) = fs::remove_file(&path) {
                eprintln!("Error: could not remove quiz.");
                ::std::process::exit(2);
            }
        }
        Ok(())
    } else {
        Err(QuizError::QuizNotFound(options.name.clone()))
    }
}


pub fn main_list() -> Result<(), QuizError> {
    let mut dirpath = get_app_dir_path();
    dirpath.push("quizzes");

    if let Ok(iter) = dirpath.read_dir() {
        let mut found_any = false;
        for entry in iter {
            if let Ok(entry) = entry {
                if let Some(stem) = entry.path().file_stem() {
                    if let Some(stem) = stem.to_str() {
                        if !found_any {
                            println!("Available quizzes:");
                            found_any = true;
                        }
                        println!("  {}", stem);
                    }
                }
            }
        }

        if !found_any {
            println!("No quizzes found.");
        }
    } else {
        println!("No quizzes found.");
    }
    Ok(())
}


impl Quiz {
    /// Take the quiz and return pairs of questions and results.
    fn take(&mut self, options: &QuizTakeOptions) -> Vec<(&Question, QuestionResult)> {
        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total_partial_correct = 0;
        let mut total_ungraded = 0;
        let mut total = 0;
        let mut aggregate_score = 0.0;

        let questions = self.choose_questions(&options);
        if questions.len() == 0 {
            println!("No questions found.");
            return Vec::new();
        }

        for (i, question) in questions.iter().enumerate() {
            println!("\n");
            if let Ok(result) = question.ask(i+1) {
                let score_option = result.score;
                results.push((*question, result));

                if let Some(score) = score_option {
                    total += 1;
                    aggregate_score += score;
                    if score == 1.0 {
                        total_correct += 1;
                    } else if score > 0.0 {
                        total_partial_correct += 1;
                    }
                } else {
                    total_ungraded += 1;
                }
            } else {
                break;
            }
        }

        if total > 0 {
            let score = (aggregate_score / (total as f64)) * 100.0;
            let score_as_str = format!("{:.1}%", score);

            print!  ("\n\n");
            print!  ("{}", "Score: ".white());
            print!  ("{}", score_as_str.cyan());
            print!  ("{}", " out of ".white());
            print!  ("{}", format!("{}", total + total_ungraded).cyan());
            if total + total_ungraded == 1 {
                println!("{}", " question".white());
            } else {
                println!("{}", " questions".white());
            }
            print!  ("  {}", format!("{}", total_correct).bright_green());
            println!("{}", " correct".white());
            print!  ("  {}", format!("{}", total_partial_correct).green());
            println!("{}", " partially correct".white());
            print!  ("  {}", format!("{}", total_ungraded).cyan());
            println!("{}", " ungraded".white());
        } else if total_ungraded > 0 {
            println!("{}", "\n\nAll questions were ungraded.".white());
        }

        results
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
        if !options.in_order {
            let mut rng = thread_rng();
            candidates.shuffle(&mut rng);
        }

        if options.best {
            candidates.sort_by(cmp_questions_best);
        } else if options.worst {
            candidates.sort_by(cmp_questions_worst);
        }

        if let Some(num_to_ask) = options.num_to_ask {
            candidates.truncate(num_to_ask);
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
        && (!options.never || q.prior_results.is_none()
            || q.prior_results.as_ref().unwrap().len() == 0)
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
    fn new(text: &str, answer: &str) -> Self {
        let answers = vec![Answer { variants: vec![String::from(answer)] }];
        Question {
            kind: QuestionKind::ShortAnswer, text: vec![String::from(text)],
            tags: Vec::new(), answer_list: answers, candidates: Vec::new(),
            prior_results: None, id: None, depends: None,
        }
    }

    /// Ask the question, get an answer, and return a `QuestionResult` object. If Ctrl+C
    /// is pressed, return an error.
    ///
    /// The `num` argument is the question number in the quiz, which is printed before
    /// the text of the question.
    fn ask(&self, num: usize) -> Result<QuestionResult, ()> {
        self.print_text(num);

        match self.kind {
            QuestionKind::ShortAnswer => {
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
            QuestionKind::Ungraded => {
                self.ask_ungraded()
            }
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ShortAnswer`.
    fn ask_short_answer(&self) -> Result<QuestionResult, ()> {
        let guess = prompt("> ")?;
        let result = guess.is_some() && self.check_any(guess.as_ref().unwrap());

        if result {
            print_correct();
        } else {
            print_incorrect(&self.answer_list[0].variants[0]);
        }

        let score = if result { 1.0 } else { 0.0 };

        if let Some(guess) = guess {
            Ok(QuestionResult::new_with_response(score, &guess.to_lowercase()))
        } else {
            Ok(QuestionResult::new(score))
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ListAnswer`.
    fn ask_list_answer(&self) -> Result<QuestionResult, ()> {
        let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
        for _ in 0..self.answer_list.len() {
            satisfied.push(false);
        }

        let mut count = 0;
        while count < self.answer_list.len() {
            if let Some(guess) = prompt("> ")? {
                let index = self.check_one(&guess);
                if index == self.answer_list.len() {
                    print_incorrect("");
                    count += 1;
                } else if satisfied[index] {
                    println!("{}", "You already said that.".white());
                } else {
                    satisfied[index] = true;
                    print_correct();
                    count += 1;
                }
            } else {
                print_incorrect("");
                break;
            }
        }

        let ncorrect = satisfied.iter().filter(|x| **x).count();
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if ncorrect < self.answer_list.len() {
            println!("{}", "\nYou missed:".white());
            for (i, correct) in satisfied.iter().enumerate() {
                if !correct {
                    println!("  {}", self.answer_list[i].variants[0]);
                }
            }
            println!(
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            );
        }
        Ok(QuestionResult::new(score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `OrderedListAnswer`.
    fn ask_ordered_list_answer(&self) -> Result<QuestionResult, ()> {
        let mut ncorrect = 0;
        for answer in self.answer_list.iter() {
            if let Some(guess) = prompt("> ")? {
                if answer.check(&guess) {
                    print_correct();
                    ncorrect += 1;
                } else {
                    print_incorrect(&answer.variants[0]);
                }
            } else {
                print_incorrect(&answer.variants[0]);
                break;
            }
        }
        let score = (ncorrect as f64) / (self.answer_list.len() as f64);
        if score < 1.0 {
            println!(
                "\n{}",
                format!(
                    "Score for this question: {}",
                    format!("{:.1}%", score * 100.0).cyan()
                ).white()
            );
        }
        Ok(QuestionResult::new(score))
    }

    /// Implementation of `ask` assuming that `self.kind` is `MultipleChoice`.
    fn ask_multiple_choice(&self) -> Result<QuestionResult, ()> {
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
            println!("     ({}) {}", i, candidate);
        }

        println!("");
        loop {
            if let Some(guess) = prompt("Enter a letter: ")? {
                if guess.len() != 1 {
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    if self.check_any(&candidates[(index - 97) as usize]) {
                        print_correct();
                        return Ok(QuestionResult::new(1.0));
                    } else {
                        print_incorrect(&answer);
                        return Ok(QuestionResult::new(0.0));
                    }
                } else {
                    continue;
                }
            } else {
                print_incorrect(&answer);
                return Ok(QuestionResult::new(0.0));
            }
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `Ungraded`.
    fn ask_ungraded(&self) -> Result<QuestionResult, ()> {
        prompt("> ")?;
        println!("\n{}", "Sample correct answer:\n".white());
        prettyprint(&self.answer_list[0].variants[0], Some("  "));
        Ok(QuestionResult {
            time_asked: chrono::Utc::now(), score: None, response: None
        })
    }

    fn print_text(&self, num: usize) {
        let mut rng = thread_rng();
        let text = self.text.choose(&mut rng).unwrap();

        let num_prefix = format!("  ({}) ", num);
        let width = textwrap::termwidth() - num_prefix.len();
        let mut lines = textwrap::wrap_iter(text, width);

        if let Some(first_line) = lines.next() {
            println!("{}{}", num_prefix.cyan(), first_line.white());
        }

        let prefix = " ".repeat(num_prefix.len());
        for line in lines {
            println!("{}{}", prefix, line.white());
        }

        print!("\n");
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


impl QuestionResult {
    fn new(score: f64) -> Self {
        QuestionResult {
            time_asked: chrono::Utc::now(),
            score: Some(score),
            response: None,
        }
    }

    fn new_with_response(score: f64, response: &str) -> Self {
        QuestionResult {
            time_asked: chrono::Utc::now(),
            score: Some(score),
            response: Some(response.to_string()),
        }
    }
}


impl QuizFilterOptions {
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
fn prompt(message: &str) -> Result<Option<String>, ()> {
    loop {
        let mut rl = rustyline::Editor::<()>::new();
        let result = rl.readline(&format!("{}", message.white()));
        match result {
            // Return immediately if the user hits Ctrl+D or Ctrl+C.
            Err(ReadlineError::Interrupted) => {
                return Err(());
            },
            Err(ReadlineError::Eof) => {
                return Ok(None);
            },
            _ => {}
        }

        let response = result.expect("Failed to read line");
        let response = response.trim();
        if response.len() > 0 {
            return Ok(Some(response.to_string()));
        }
    }
}


/// Print `message` to standard output, breaking lines according to the current width
/// of the terminal. If `prefix` is `Some(string)`, then prepend `string` (usually
/// whitespace for indentation) to every line.
fn prettyprint(message: &str, prefix: Option<&str>) {
    let prefix = prefix.unwrap_or("");
    let filled = textwrap::fill(message, textwrap::termwidth() - prefix.len());
    let mut indented = textwrap::indent(&filled, prefix);
    // textwrap::indent will append unwanted newlines sometimes, which we remove here.
    if !message.ends_with("\n") && indented.ends_with("\n") {
        indented = indented.trim_end().to_string();
    }
    println!("{}", indented);
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
    QuizOptions::from_args()
}


/// Print a list of tags.
fn list_tags(quiz: &Quiz) {
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
        println!("No questions have been assigned tags.");
    } else {
        println!("Available tags:");

        let mut tags_in_order: Vec<(&str, u32)> = tags.into_iter().collect();
        tags_in_order.sort();
        for (tag, count) in tags_in_order.iter() {
            println!("  {} ({})", tag, count);
        }
    }
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
fn save_results(name: &str, results: &Vec<(&Question, QuestionResult)>) -> Result<(), QuizError> {
    require_app_dir_path()?;

    // Load old data, if it exists.
    let path = get_results_path(name);
    let data = fs::read_to_string(&path);
    let mut hash: HashMap<&str, Vec<QuestionResult>> = match data {
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
    for (q, qr) in results.iter() {
        let qtext = q.text[0].as_str();
        if !hash.contains_key(qtext) {
            hash.insert(qtext, Vec::new());
        }
        hash.get_mut(qtext).unwrap().push((*qr).clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .map_err(QuizError::Json)?;
    fs::write(&path, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(path.clone())))?;
    Ok(())
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


fn load_results(name: &str) -> Result<StoredResults, QuizError> {
    let path = get_results_path(name);

    match fs::read_to_string(&path) {
        Ok(data) => {
            serde_json::from_str(&data).map_err(QuizError::Json)
        },
        Err(_) => {
            Ok(HashMap::new())
        }
    }
}


/// Load a `Quiz` object given its name.
fn load_quiz(name: &str) -> Result<Quiz, QuizError> {
    let path = get_quiz_path(name);
    let data = fs::read_to_string(path)
        .or(Err(QuizError::QuizNotFound(name.to_string())))?;

    let mut quiz = load_quiz_from_json(&data)?;
    // Attach previous results to the `Question` objects.
    let old_results = load_results(name)?;
    for question in quiz.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.text[0]) {
            question.prior_results = Some(results.clone());
        }
    }
    Ok(quiz)
}


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
                            expand_question_json(&question)
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


type JSONMap = serde_json::Map<String, serde_json::Value>;


/// Given a JSON object in the disk format, return an equivalent JSON object in the
/// format that the deserialization library understands (i.e., a format that is
/// isomorphic to the fields of the `Question` struct).
fn expand_question_json(question: &JSONMap) -> JSONMap {
    let mut ret = question.clone();

    // Only multiple-choice questions require the `candidates` field, so other
    // questions can omit them.
    if !ret.contains_key("candidates") {
        ret.insert(String::from("candidates"), serde_json::json!([]));
    }

    // The `kind` field defaults to "ShortAnswer".
    if !ret.contains_key("kind") {
        ret.insert(String::from("kind"), serde_json::json!("ShortAnswer"));
    }

    // The `tags` field defaults to an empty array.
    if !ret.contains_key("tags") {
        ret.insert(String::from("tags"), serde_json::json!([]));
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


/// Compare two tuples with floating-point numbers.
///
/// The comparison is reversed to produce descending order when sorting.
///
/// Courtesy of https://users.rust-lang.org/t/sorting-vector-of-vectors-of-f64/16264
fn cmp_f64_tuple_reversed(a: &(f64, usize, String), b: &(f64, usize, String)) -> Ordering {
    if a.0 < b.0 {
        return Ordering::Greater;
    } else if a.0 > b.0 {
        return Ordering::Less;
    } else {
        if a.1 < b.1 {
            return Ordering::Greater;
        } else if a.1 > b.1 {
            return Ordering::Less;
        } else {
            if a.2 < b.2 {
                return Ordering::Greater;
            } else if a.2 > b.2 {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }
    }
}


/// Comparison function that sorts an array of `Question` objects such that the
/// questions with the highest previous scores come first.
fn cmp_questions_best(a: &&Question, b: &&Question) -> Ordering {
    let a_score = (*a).prior_results.as_ref().map(aggregate_results)
        .unwrap_or(Some(0.0)).unwrap_or(0.0);
    let b_score = (*b).prior_results.as_ref().map(aggregate_results)
        .unwrap_or(Some(0.0)).unwrap_or(0.0);

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


/// Print a message for correct answers.
fn print_correct() {
    println!("{}", "Correct!".green());
}


/// Print a message for an incorrect answer, indicating that `answer` was the correct
/// answer.
fn print_incorrect(answer: &str) {
    if answer.len() > 0 {
        let message = &format!(
            "{} The correct answer was {}.", "Incorrect.".red(), answer.green()
        );
        prettyprint(message, None);
    } else {
        println!("{}", "Incorrect.".red());
    }
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
            prior_results: None,
            tags: Vec::new(),
            id: None,
            depends: None,
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
            prior_results: None,
            tags: Vec::new(),
            id: None,
            depends: None,
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
            prior_results: None,
            tags: Vec::new(),
            id: None,
            depends: None,
        };

        assert_eq!(*output, expected_output);
    }
}
