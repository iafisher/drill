/**
 * Implementation of the popquiz application.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;

use argparse::{ArgumentParser, Collect, Store, StoreTrue};
use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rustyline::error::ReadlineError;
use serde::{Serialize, Deserialize};


/// Represents an entire quiz.
#[derive(Serialize, Deserialize, Debug)]
pub struct Quiz {
    pub questions: Vec<Question>,
}


/// Holds the command-line configuration for the application. See `parse_options` for
/// the meaning of each field.
pub struct QuizOptions {
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub exclude: Vec<String>,
    pub num_to_ask: i16,
    pub list_tags: bool,
    pub do_save_results: bool,
    pub count: bool,
    pub no_color: bool,
    pub in_order: bool,
    pub delete_results: bool,
    pub force_delete_results: bool,
    pub print_results: bool,
}


/// Represents a question.
#[derive(Serialize, Deserialize, Debug)]
pub struct Question {
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
}


/// An enumeration for the `kind` field of `Question` objects.
#[derive(Serialize, Deserialize, Debug)]
enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice,
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
pub struct QuestionResult {
    pub time_asked: chrono::DateTime<chrono::Utc>,
    pub correct: bool,
}


impl Quiz {
    /// Construct a new `Quiz` object from a vector of `Questions`.
    pub fn new(questions: Vec<Question>) -> Self {
        Quiz { questions }
    }

    /// Take the quiz and return pairs of questions and results.
    pub fn take(&mut self, options: &QuizOptions) -> Vec<(&Question, QuestionResult)> {
        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total = 0;

        let questions = self.choose_questions(&options);
        if questions.len() == 0 {
            println!("No questions found.");
            return Vec::new();
        }

        for question in questions.iter() {
            println!("\n");
            let correct = question.ask();
            let result = QuestionResult {
                time_asked: chrono::Utc::now(),
                correct,
            };
            results.push((*question, result));

            total += 1;
            if correct {
                total_correct += 1;
            }
        }

        if total > 0 {
            let score = (total_correct as f64) / (total as f64) * 100.0;
            println!("\n{} correct out of {} ({:.1}%).", total_correct, total, score);
        }

        results
    }

    /// Return the questions filtered by the given command-line options (e.g., `--tag`
    /// and `--exclude`). Note that the `-n` flag is not applied, unlike in the
    /// `choose_questions` method.
    pub fn filter_questions(&self, options: &QuizOptions) -> Vec<&Question> {
        let mut candidates = Vec::new();
        for question in self.questions.iter() {
            if self.filter_question(question, options) {
                candidates.push(question);
            }
        }
        candidates
    }

    /// Choose a set of questions, filtered by the command-line options.
    fn choose_questions(&self, options: &QuizOptions) -> Vec<&Question> {
        let mut candidates = self.filter_questions(options);
        if !options.in_order {
            let mut rng = thread_rng();
            candidates.shuffle(&mut rng);
        }
        if options.num_to_ask > 0 {
            candidates.truncate(options.num_to_ask as usize);
        }
        candidates
    }

    /// Return `true` if `q` satisfies the constraints in `options`.
    fn filter_question(&self, q: &Question, options: &QuizOptions) -> bool {
        // Either no tags were specified, or `q` has all the specified tags.
        (options.tags.len() == 0 || options.tags.iter().all(|tag| q.tags.contains(tag)))
            // `q` must not have any excluded tags.
            && options.exclude.iter().all(|tag| !q.tags.contains(tag))
    }
}


impl Question {
    /// Ask the question, get an answer, and return `true` if the user got the question
    /// right.
    pub fn ask(&self) -> bool {
        let mut rng = thread_rng();
        let text = self.text.choose(&mut rng).unwrap();
        prettyprint(&format!("{}\n", text.white()), Some("  "));

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
            }
        }
    }

    /// Implementation of `ask` assuming that `self.kind` is `ShortAnswer`.
    fn ask_short_answer(&self) -> bool {
        let guess = prompt("> ");
        let result = guess.is_some() && self.check_any(&guess.unwrap());
        if result {
            print_correct();
        } else {
            print_incorrect(&self.answer_list[0].variants[0]);
        }
        result
    }

    /// Implementation of `ask` assuming that `self.kind` is `ListAnswer`.
    fn ask_list_answer(&self) -> bool {
        let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
        for _ in 0..self.answer_list.len() {
            satisfied.push(false);
        }

        let mut count = 0;
        while count < self.answer_list.len() {
            if let Some(guess) = prompt("> ") {
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

        let all_correct = satisfied.iter().all(|x| *x);
        if !all_correct {
            println!("{}", "\nYou missed:".white());
            for (i, correct) in satisfied.iter().enumerate() {
                if !correct {
                    println!("  {}", self.answer_list[i].variants[0].white());
                }
            }
        }
        all_correct
    }

    /// Implementation of `ask` assuming that `self.kind` is `OrderedListAnswer`.
    fn ask_ordered_list_answer(&self) -> bool {
        let mut correct = true;
        for answer in self.answer_list.iter() {
            if let Some(guess) = prompt("> ") {
                if answer.check(&guess) {
                    print_correct();
                } else {
                    print_incorrect(&answer.variants[0]);
                    correct = false;
                }
            } else {
                print_incorrect(&answer.variants[0]);
                correct = false;
                break;
            }
        }
        correct
    }

    /// Implementation of `ask` assuming that `self.kind` is `MultipleChoice`.
    fn ask_multiple_choice(&self) -> bool {
        let mut candidates = self.candidates.clone();

        let mut rng = thread_rng();
        candidates.shuffle(&mut rng);
        candidates.truncate(3);
        candidates.push(self.answer_list[0].variants[0].clone());
        candidates.shuffle(&mut rng);

        for (i, candidate) in "abcd".chars().zip(candidates.iter()) {
            println!("  ({}) {}", i, candidate);
        }

        println!("");
        loop {
            if let Some(guess) = prompt("Enter a letter: ") {
                if guess.len() != 1 {
                    continue;
                }

                let index = guess.to_ascii_lowercase().as_bytes()[0];
                if 97 <= index && index < 101 {
                    if self.check_any(&candidates[(index - 97) as usize]) {
                        print_correct();
                        return true;
                    } else {
                        print_incorrect(&self.answer_list[0].variants[0]);
                        return false;
                    }
                } else {
                    continue;
                }
            } else {
                print_incorrect(&self.answer_list[0].variants[0]);
                return false;
            }
        }
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
    pub fn check(&self, guess: &str) -> bool {
        for variant in self.variants.iter() {
            if variant.to_lowercase() == guess.to_lowercase() {
                return true;
            }
        }
        false
    }
}


/// Display a prompt and read a line from standard input continually until the user
/// enters a line with at least one non-whitespace character. If the user presses Ctrl+D
/// then None is returned. If the user pressed Ctrl+C then the entire application exits.
/// Otherwise, `Some(line)` is returned where `line` is the last line of input the user
/// entered without leading and trailing whitespace.
fn prompt(message: &str) -> Option<String> {
    loop {
        let mut rl = rustyline::Editor::<()>::new();
        let result = rl.readline(&format!("{}", message.white()));
        match result {
            // Exit if the user hits Ctrl+C.
            Err(ReadlineError::Interrupted) => {
                ::std::process::exit(2);
            },
            // Return immediately if the user hits Ctrl+D.
            Err(ReadlineError::Eof) => {
                return None;
            },
            _ => {}
        }

        let response = result.expect("Failed to read line");
        let response = response.trim();
        if response.len() > 0 {
            return Some(response.to_string());
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
pub fn yesno(message: &str) -> bool {
    let response = prompt(message);
    response.is_some() && response.unwrap().trim_start().to_lowercase().starts_with("y")
}


/// Parse command-line arguments.
pub fn parse_options() -> QuizOptions {
    let mut options = QuizOptions {
        paths: Vec::new(), tags: Vec::new(), exclude: Vec::new(), num_to_ask: -1,
        list_tags: false, do_save_results: false, count: false, no_color: false,
        in_order: false, delete_results: false, force_delete_results: false,
        print_results: false,
    };
    {
        let mut parser = ArgumentParser::new();
        parser.set_description("Take a pop quiz from the command line.");

        parser.refer(&mut options.paths).add_argument(
            "quizzes", Collect, "Paths to the quiz files."
        ).required();

        parser.refer(&mut options.delete_results).add_option(
            &["--delete-results"], StoreTrue, "Clear cached results of previous attempts."
        );

        // Make sure to maintain alphabetical order of flags.
        parser.refer(&mut options.count).add_option(
            &["--count"], StoreTrue, "Count the number of questions."
        );

        parser.refer(&mut options.exclude).add_option(
            &["--exclude"], Collect, "Exclude questions by tag."
        );

        parser.refer(&mut options.force_delete_results).add_option(
            &["--force-delete-results"],
            StoreTrue,
            "Clear cached results of previous attempts, without prompting for confirmation."
        );

        parser.refer(&mut options.in_order).add_option(
            &["--in-order"], StoreTrue, "Ask questions in order."
        );

        parser.refer(&mut options.list_tags).add_option(
            &["--list-tags"], StoreTrue, "List all available tags."
        );

        parser.refer(&mut options.num_to_ask).add_option(
            &["-n"], Store, "Number of questions to ask."
        );

        parser.refer(&mut options.no_color).add_option(
            &["--no-color"], StoreTrue, "Turn off ANSI color in output."
        );

        parser.refer(&mut options.print_results).add_option(
            &["-r", "--results"], StoreTrue, "Print a report of previous results."
        );

        parser.refer(&mut options.do_save_results).add_option(
            &["--save"], StoreTrue, "Save quiz results without prompting."
        );

        parser.refer(&mut options.tags).add_option(
            &["--tag"], Collect, "Filter questions by tag."
        );

        parser.parse_args_or_exit();
    }

    if options.delete_results || options.force_delete_results {
        let which = if options.delete_results {
            "--delete-results"
        } else {
            "--force-delete-results"
        };

        if options.count {
            incompatible(&which, "--count");
        } else if options.exclude.len() > 0 {
            incompatible(&which, "--exclude");
        } else if options.in_order {
            incompatible(&which, "--in-order");
        } else if options.list_tags {
            incompatible(&which, "--list-tags");
        } else if options.num_to_ask != -1 {
            incompatible(&which, "-n");
        } else if options.print_results {
            incompatible(&which, "--results");
        } else if options.do_save_results {
            incompatible(&which, "--save");
        } else if options.tags.len() > 0 {
            incompatible(&which, "--tag");
        }
    }

    if options.count {
        if options.in_order {
            incompatible("--count", "--in-order");
        } else if options.list_tags {
            incompatible("--count", "--list-tags");
        } else if options.num_to_ask != -1 {
            incompatible("--count", "-n");
        } else if options.print_results {
            incompatible("--count", "--results");
        } else if options.do_save_results {
            incompatible("--count", "--save");
        }
    }

    if options.list_tags {
        if options.exclude.len() > 0 {
            incompatible("--list-tags", "--exclude");
        } else if options.in_order {
            incompatible("--list-tags", "--in-order");
        } else if options.num_to_ask != -1 {
            incompatible("--list-tags", "-n");
        } else if options.print_results {
            incompatible("--list-tags", "--results");
        } else if options.do_save_results {
            incompatible("--list-tags", "--save");
        } else if options.tags.len() > 0 {
            incompatible("--list-tags", "--tag");
        }
    }

    if options.print_results {
        if options.exclude.len() > 0 {
            incompatible("--results", "--exclude");
        } else if options.in_order {
            incompatible("--results", "--in-order");
        } else if options.num_to_ask != -1 {
            incompatible("--results", "-n");
        } else if options.do_save_results {
            incompatible("--results", "--save");
        } else if options.tags.len() > 0 {
            incompatible("--results", "--tag");
        }
    }

    options
}


/// Print all the tags that are attached to any question in `quiz`.
pub fn list_tags(quiz: &Quiz) {
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
pub fn save_results(results: &Vec<(&Question, QuestionResult)>) {
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
pub fn delete_results() {
    let path = get_results_path();
    fs::remove_file(&path).expect("Unable to remove file");
    println!("Successfully deleted {}", path.to_str().unwrap());
}


pub fn print_results() {
    let path = get_results_path();

    match fs::read_to_string(&path) {
        Ok(data) => {
            match serde_json::from_str(&data) {
                Ok(results) => {
                    print_results_from_json_value(results);
                },
                Err(e) => {
                    eprintln!("Error: could not deserialize quiz results.");
                    eprintln!("  Reason: {}", e);
                }
            }
        },
        Err(e) => {
            eprintln!(
                "Error: could not open results file at {} for reading.",
                path.to_str().unwrap()
            );
            eprintln!("  Reason: {}", e);
        }
    }
}


/// Load a `Quiz` object from the file at `path`.
pub fn load_quiz(path: &str) -> Quiz {
    let data = fs::read_to_string(path)
        .expect("Unable to read from quiz file");
    let mut quiz_as_json: serde_json::Value = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON");

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

    // TODO: Can I convert from Value to my custom type without serializing the whole
    // thing to a string?
    return serde_json::from_str(&quiz_as_json.to_string())
        .expect("Unable to deserialize expanded JSON to Quiz object");
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


type ResultsMap = HashMap<String, Vec<QuestionResult>>;

/// Helper function for `print_results` that operates directly on the JSON.
fn print_results_from_json_value(value: serde_json::Value) {
    match serde_json::from_value::<ResultsMap>(value) {
        Ok(results) => {
            let mut aggregated: Vec<(f64, String)> = Vec::new();
            for (key, result) in results.iter() {
                aggregated.push((aggregate_results(&result), key.clone()));
            }

            aggregated.sort_by(cmp_f64_tuple_reversed);

            for (score, question) in aggregated.iter() {
                println!("{:>5.1}%  {}", score, question);
            }
        },
        Err(e) => {
            eprintln!("Error: could not parse quiz results.");
            eprintln!("Reason: {}", e);
        }
    }
}


/// Return the percentage of correct responses in the vector of results.
fn aggregate_results(results: &Vec<QuestionResult>) -> f64 {
    let mut count = 0;
    for result in results.iter() {
        if result.correct {
            count += 1;
        }
    }

    if results.len() == 0 {
        // Just to be safe, although this should never happen.
        100.0
    } else {
        100.0 * (count as f64) / (results.len() as f64)
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


/// Display a message for incompatible command-line flags, and exit.
fn incompatible(flag1: &str, flag2: &str) {
    eprintln!("{} and {} are incompatible.", flag1, flag2);
    ::std::process::exit(1);
}
