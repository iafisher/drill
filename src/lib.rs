/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;

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
#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice, Ungraded,
}


/// Represents an answer.
#[derive(Serialize, Deserialize, Debug)]
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
}

#[derive(StructOpt)]
pub struct QuizTakeOptions {
    /// Paths to the quiz files.
    paths: Vec<String>,
    /// Only include questions with the given tag.
    #[structopt(long = "tag")]
    tags: Vec<String>,
    /// Exclude questions with the given tag.
    #[structopt(long = "exclude")]
    exclude: Vec<String>,
    /// Limit the total number of questions.
    #[structopt(short = "n", default_value = "-1")]
    num_to_ask: i16,
    /// Save results without prompting.
    #[structopt(long = "save")]
    save: bool,
    /// Do not emit colorized output.
    #[structopt(long = "no-color")]
    no_color: bool,
    /// Ask the questions in the order they appear in the quiz file.
    #[structopt(long = "in-order")]
    in_order: bool,
    /// Only ask questions that have never been asked before.
    #[structopt(long = "never")]
    never: bool,
    /// Filter by keyword.
    #[structopt(short = "k", long = "keyword")]
    keywords: Vec<String>,
}

#[derive(StructOpt)]
pub struct QuizCountOptions {
    /// Paths to the quiz files.
    paths: Vec<String>,
    /// Only include questions with the given tag.
    #[structopt(long = "tag")]
    tags: Vec<String>,
    /// Exclude questions with the given tag.
    #[structopt(long = "exclude")]
    exclude: Vec<String>,
    /// List tags instead of counting questions.
    #[structopt(long = "list-tags")]
    list_tags: bool,
    /// Only count questions that have never been asked before.
    #[structopt(long = "never")]
    never: bool,
    /// Filter by keyword.
    #[structopt(short = "k", long = "keyword")]
    keywords: Vec<String>,
}

impl From<QuizCountOptions> for QuizTakeOptions {
    fn from(options: QuizCountOptions) -> Self {
        QuizTakeOptions {
            paths: options.paths, tags: options.tags, exclude: options.exclude,
            num_to_ask: -1, save: false, no_color: false, in_order: false,
            never: options.never, keywords: options.keywords,
        }
    }
}

#[derive(StructOpt)]
pub struct QuizResultsOptions {
    #[structopt(long = "--delete")]
    delete_results: bool,
    #[structopt(long = "--force-delete")]
    force_delete_results: bool,
}


// One main function for each subcommand.


/// The main function for the `take` subcommand.
pub fn main_take(options: QuizTakeOptions) {
    if options.no_color {
        colored::control::set_override(false);
    }

    let mut quiz = load_quizzes(&options.paths);
    let results = quiz.take(&options);
    if results.len() > 0 && (options.save || yesno("\nSave results? ")) {
        save_results(&results);
    }
}


/// The main function for the `count` subcommand.
pub fn main_count(options: QuizCountOptions) {
    let quiz = load_quizzes(&options.paths);

    if options.list_tags {
        list_tags(&quiz);
    } else {
        let filtered = quiz.filter_questions(&QuizTakeOptions::from(options));
        println!("{}", filtered.len());
    }
}


/// The main function for the `results` subcommand.
pub fn main_results(options: QuizResultsOptions) {
    if options.delete_results || options.force_delete_results {
        let prompt = "Are you sure you want to delete all previous results? ";
        if options.force_delete_results || yesno(&prompt) {
            delete_results();
        }
    } else {
        let results = load_results();
        let mut aggregated: Vec<(f64, String)> = Vec::new();
        for (key, result) in results.iter() {
            aggregated.push((aggregate_results(&result), key.clone()));
        }

        aggregated.sort_by(cmp_f64_tuple_reversed);

        for (score, question) in aggregated.iter() {
            let first_prefix = format!("{:>5.1}%  ", score);
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
    }
}


impl Quiz {
    /// Construct a new `Quiz` object from a vector of `Questions`.
    fn new(questions: Vec<Question>) -> Self {
        Quiz { questions }
    }

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
    /// and `--exclude`). Note that the `-n` flag is not applied, unlike in the
    /// `choose_questions` method.
    fn filter_questions(&self, options: &QuizTakeOptions) -> Vec<&Question> {
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
        let mut candidates = self.filter_questions(options);
        if !options.in_order {
            let mut rng = thread_rng();
            candidates.shuffle(&mut rng);
        }
        if options.num_to_ask > 0 {
            candidates.truncate(options.num_to_ask as usize);
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
fn filter_question(q: &Question, options: &QuizTakeOptions) -> bool {
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


impl QuizTakeOptions {
    fn new() -> Self {
        QuizTakeOptions {
            paths: Vec::new(), tags: Vec::new(), exclude: Vec::new(), num_to_ask: -1,
            save: false, no_color: false, in_order: false, never: false,
            keywords: Vec::new(),
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
fn save_results(results: &Vec<(&Question, QuestionResult)>) {
    // Create the data directory if it does not already exist.
    let dirpath = get_results_dir_path();
    if !dirpath.as_path().exists() {
        let emsg = format!(
            "Unable to create data directory at {}", dirpath.to_str().unwrap()
        );
        fs::create_dir(&dirpath).expect(&emsg);
    }

    // Load old data, if it exists.
    let path = get_results_path();
    let data = fs::read_to_string(&path);
    let mut hash: HashMap<&str, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .expect("Unable to deserialize JSON to results object")
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
        .expect("Unable to serialize results object to JSON");
    fs::write(&path, serialized_results)
        .expect("Unable to write to quiz file");

    println!("Results saved to {}", path.to_str().unwrap());
}


/// Delete previously saved results.
fn delete_results() {
    let path = get_results_path();
    fs::remove_file(&path).expect("Unable to remove file");
    println!("Successfully deleted {}", path.to_str().unwrap());
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


fn load_results() -> StoredResults {
    let path = get_results_path();

    match fs::read_to_string(&path) {
        Ok(data) => {
            match serde_json::from_str(&data) {
                Ok(results) => {
                    results
                },
                Err(e) => {
                    eprintln!("Error: could not parse {}.", path.to_str().unwrap());
                    eprintln!("  Reason: {}", e);
                    ::std::process::exit(2);
                }
            }
        },
        Err(e) => {
            eprintln!("Error: could not open {} for reading.", path.to_str().unwrap());
            eprintln!("  Reason: {}", e);
            ::std::process::exit(2);
        }
    }
}


/// Load a single `Quiz` object from a vector of paths to quiz files.
fn load_quizzes(paths: &Vec<String>) -> Quiz {
    let results = load_results();

    let mut master_list = Vec::new();
    for path in paths.iter() {
        match load_quiz(path, &results) {
            Ok(mut quiz) => {
                master_list.append(&mut quiz.questions);
            },
            Err(e) => {
                eprintln!("Error on {}: {}", path, e);
                ::std::process::exit(2);
            }
        }
    }
    Quiz::new(master_list)
}


/// Load a `Quiz` object from the file at `path`.
fn load_quiz(path: &str, old_results: &StoredResults) -> Result<Quiz, Box<::std::error::Error>> {
    let data = fs::read_to_string(path)?;
    let mut quiz_as_json: serde_json::Value = serde_json::from_str(&data)?;

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

    let mut ret: Quiz = serde_json::from_value(quiz_as_json)?;

    // Attach previous results to the `Question` objects.
    for question in ret.questions.iter_mut() {
        if let Some(results) = old_results.get(&question.text[0]) {
            question.prior_results = Some(results.clone());
        }
    }

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


/// Return the percentage of correct responses in the vector of results.
fn aggregate_results(results: &Vec<QuestionResult>) -> f64 {
    let mut sum = 0.0;
    for result in results.iter() {
        if let Some(score) = result.score {
            sum += score;
        }
    }

    if results.len() == 0 {
        // Just to be safe, although this should never happen.
        100.0
    } else {
        100.0 * (sum / (results.len() as f64))
    }
}


/// Compare two tuples with floating-point numbers.
///
/// The comparison is reversed to produce descending order when sorting.
///
/// Courtesy of https://users.rust-lang.org/t/sorting-vector-of-vectors-of-f64/16264
fn cmp_f64_tuple_reversed(a: &(f64, String), b: &(f64, String)) -> Ordering {
    if a.0 < b.0 {
        return Ordering::Greater;
    } else if a.0 > b.0 {
        return Ordering::Less;
    } else {
        if a.1 < b.1 {
            return Ordering::Greater;
        } else if a.1 > b.1 {
            return Ordering::Less;
        }
        return Ordering::Equal;
    }
}


/// Return the path to the file where quiz results are stored.
fn get_results_path() -> ::std::path::PathBuf {
    let mut dirpath = get_results_dir_path();
    dirpath.push("results.json");
    dirpath
}


/// Return the path to the directory where quiz results are stored.
fn get_results_dir_path() -> ::std::path::PathBuf {
    let mut dirpath = dirs::data_dir().unwrap();
    dirpath.push("iafisher_popquiz");
    dirpath
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_filter_by_tag() {
        let mut q = Question::new("What is the capital of China", "Beijing");
        q.tags.push(String::from("geography"));

        let mut options = QuizTakeOptions::new();
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

        let mut options = QuizTakeOptions::new();
        options.exclude.push(String::from("geography"));
        assert!(!filter_question(&q, &options));
    }

    #[test]
    fn can_filter_by_keyword() {
        let q = Question::new("What is the capital of China", "Beijing");

        let mut options = QuizTakeOptions::new();
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
        let input_as_json = serde_json::json!(
            {"text": "When did WW2 start?", "answer": "1939"}
        );
        let input = input_as_json.as_object().unwrap();
        let output = expand_question_json(input);

        let expected_output_as_json = serde_json::json!(
            {
                "kind": "ShortAnswer",
                "text": ["When did WW2 start?"],
                "answer_list": [
                    {"variants": ["1939"]}
                ],
                "tags": [],
                "candidates": [],
            }
        );
        let expected_output = expected_output_as_json.as_object().unwrap();

        assert_eq!(output, *expected_output);
    }

    #[test]
    fn can_expand_list_answer_json() {
        let input_as_json = serde_json::json!(
            {
                "kind": "ListAnswer",
                "text": "List the four countries of the United Kingdom.",
                "answer_list": [
                    "England", "Scotland", ["Northern Ireland", "N. Ireland"], "Wales"
                ],
            }
        );
        let input = input_as_json.as_object().unwrap();
        let output = expand_question_json(input);

        let expected_output_as_json = serde_json::json!(
            {
                "kind": "ListAnswer",
                "text": ["List the four countries of the United Kingdom."],
                "answer_list": [
                    {"variants": ["England"]},
                    {"variants": ["Scotland"]},
                    {"variants": ["Northern Ireland", "N. Ireland"]},
                    {"variants": ["Wales"]},
                ],
                "tags": [],
                "candidates": [],
            }
        );
        let expected_output = expected_output_as_json.as_object().unwrap();

        assert_eq!(output, *expected_output);
    }

    #[test]
    fn can_expand_multiple_choice_json() {
        let input_as_json = serde_json::json!(
            {
                "kind": "MultipleChoice",
                "text": "What language is spoken in Cambodia?",
                "candidates": ["Thai", "French", "Vietnamese", "Burmese", "Tagalog"],
                "answer": "Khmer"
            }
        );
        let input = input_as_json.as_object().unwrap();
        let output = expand_question_json(input);

        let expected_output_as_json = serde_json::json!(
            {
                "kind": "MultipleChoice",
                "text": ["What language is spoken in Cambodia?"],
                "candidates": ["Thai", "French", "Vietnamese", "Burmese", "Tagalog"],
                "answer_list": [
                    {"variants": ["Khmer"]},
                ],
                "tags": [],
            }
        );
        let expected_output = expected_output_as_json.as_object().unwrap();

        assert_eq!(output, *expected_output);
    }
}
