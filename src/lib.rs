use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use argparse::{ArgumentParser, Collect, Store, StoreTrue};
use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer, MultipleChoice,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Answer {
    pub variants: Vec<String>,
}

impl Answer {
    pub fn check(&self, guess: &str) -> bool {
        for variant in self.variants.iter() {
            if variant.to_lowercase() == guess.to_lowercase() {
                return true;
            }
        }
        false
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Question {
    pub kind: QuestionKind,
    pub text: Vec<String>,
    pub topic: String,
    pub answer_list: Vec<Answer>,
    pub candidates: Vec<String>,
}

impl Question {
    pub fn ask(&self) -> bool {
        let mut rng = thread_rng();
        prettyprint(&format!("{}\n", self.text.choose(&mut rng).unwrap().white()));

        match self.kind {
            QuestionKind::ShortAnswer => {
                let guess = prompt("> ");
                let result = self.check_any(&guess);
                if result {
                    print_correct();
                } else {
                    print_incorrect(&self.answer_list[0].variants[0]);
                }
                return result;
            },
            QuestionKind::ListAnswer => {
                let mut satisfied = Vec::<bool>::with_capacity(self.answer_list.len());
                for _ in 0..self.answer_list.len() {
                    satisfied.push(false);
                }

                let mut count = 0;
                while count < self.answer_list.len() {
                    let guess = prompt("> ");
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
                return all_correct;
            }
            QuestionKind::OrderedListAnswer => {
                let mut correct = true;
                for answer in self.answer_list.iter() {
                    let guess = prompt("> ");
                    if answer.check(&guess) {
                        print_correct();
                    } else {
                        print_incorrect(&answer.variants[0]);
                        correct = false;
                    }
                }
                return correct;
            }
            QuestionKind::MultipleChoice => {
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
                    let guess = prompt("Enter a letter: ");
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
                }
            }
        }
    }

    fn check_any(&self, guess: &str) -> bool {
        for answer in self.answer_list.iter() {
            if answer.check(guess) {
                return true;
            }
        }
        false
    }

    fn check_one(&self, guess: &str) -> usize {
        for (i, answer) in self.answer_list.iter().enumerate() {
            if answer.check(guess) {
                return i;
            }
        }
        self.answer_list.len()
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuestionResult {
    pub time_asked: chrono::DateTime<chrono::Utc>,
    pub correct: bool,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Quiz {
    pub questions: Vec<Question>,
}

impl Quiz {
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

    pub fn filter_questions(&self, options: &QuizOptions) -> Vec<&Question> {
        let mut candidates = Vec::new();
        for question in self.questions.iter() {
            if !self.filter_question(question, options) {
                candidates.push(question);
            }
        }
        candidates
    }

    fn choose_questions(&self, options: &QuizOptions) -> Vec<&Question> {
        let mut rng = thread_rng();

        let mut candidates = self.filter_questions(options);
        candidates.shuffle(&mut rng);
        if options.num_to_ask > 0 {
            candidates.truncate(options.num_to_ask as usize);
        }
        candidates
    }

    fn filter_question(&self, q: &Question, options: &QuizOptions) -> bool {
        options.topic.len() > 0 && q.topic != options.topic
    }
}


pub fn prompt(message: &str) -> String {
    let mut rl = rustyline::Editor::<()>::new();
    let response = rl.readline(&format!("{}", message.white()))
        .expect("Failed to read line");

    // If the string is completely empty, then the user hit Ctrl+D and we should exit.
    // A blank line is indicated by "\n" rather than "".
    if response.len() == 0 {
        println!("");
        ::std::process::exit(2);
    }

    response.trim().to_string()
}


pub fn prettyprint(message: &str) {
    println!("{}", textwrap::fill(message, textwrap::termwidth()));
}


pub fn yesno(message: &str) -> bool {
    let response = prompt(message);
    response.trim_start().to_lowercase().starts_with("y")
}


pub struct QuizOptions {
    pub paths: Vec<String>,
    pub topic: String,
    pub num_to_ask: i16,
    pub list_topics: bool,
    pub save_results: bool,
    pub count: bool,
    pub no_color: bool,
}


pub fn parse_options() -> QuizOptions {
    let mut paths = Vec::new();
    let mut topic = String::new();
    let mut num_to_ask = -1;
    let mut list_topics = false;
    let mut save_results = false;
    let mut count = false;
    let mut no_color = false;
    {
        let mut parser = ArgumentParser::new();
        parser.set_description("Take a pop quiz from the command line.");

        parser.refer(&mut paths)
            .add_argument("quizzes", Collect, "Paths to the quiz files.").required();

        parser.refer(&mut topic)
            .add_option(&["--topic"], Store, "Restrict questions to a certain topic.");

        parser.refer(&mut num_to_ask)
            .add_option(&["-n"], Store, "Number of questions to ask.");

        parser.refer(&mut list_topics)
            .add_option(&["--list-topics"], StoreTrue, "List all available topics.");

        parser.refer(&mut save_results)
            .add_option(&["--save"], StoreTrue, "Save quiz results without prompting.");

        parser.refer(&mut count)
            .add_option(
                &["--count"], StoreTrue, "Count the number of questions."
            );

        parser.refer(&mut no_color)
            .add_option(&["--no-color"], StoreTrue, "Turn off ANSI color in output.");

        parser.parse_args_or_exit();
    }
    QuizOptions { paths, topic, num_to_ask, list_topics, save_results, count, no_color }
}


pub fn list_topics(quiz: &Quiz) {
    let mut topics = HashSet::new();
    for question in quiz.questions.iter() {
        if question.topic.len() > 0 {
            topics.insert(question.topic.as_str());
        }
    }

    if topics.len() == 0 {
        println!("No questions have been assigned topics.");
    } else {
        println!("Available topics:");
        for topic in topics.iter() {
            println!("  {}", topic);
        }
    }
}


pub fn save_results(path: &str, results: &Vec<(&Question, QuestionResult)>) {
    let data = fs::read_to_string(path);
    let mut hash: HashMap<&str, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .expect("Unable to deserialize JSON to results object")
        },
        Err(_) => {
            HashMap::new()
        }
    };

    for (q, qr) in results.iter() {
        let qtext = q.text[0].as_str();
        if !hash.contains_key(qtext) {
            hash.insert(qtext, Vec::new());
        }
        hash.get_mut(qtext).unwrap().push((*qr).clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .expect("Unable to serialize results object to JSON");
    fs::write(path, serialized_results)
        .expect("Unable to write to quiz file");
    println!("Results saved to {}.", path);
}


pub fn derive_result_path(path: &str) -> String {
    let ext = match Path::new(path).extension() {
        Some(ext) => ext.to_str().unwrap(),
        None => "",
    };

    if ext.len() > 0 {
        let stem: String = path.chars().take(path.len() - ext.len() - 1).collect();
        format!("{}_results.{}", stem, ext)
    } else {
        String::from(path)
    }
}


pub fn load_quiz(path: &str) -> Quiz {
    let data = fs::read_to_string(path)
        .expect("Unable to read from quiz file");
    let mut quiz_as_json: serde_json::Value = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON");

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
fn expand_question_json(question: &JSONMap) -> JSONMap {
    let mut ret = question.clone();

    // Only multiple-choice questions require the `candidates` field, so other
    // questions can omit them.
    if !ret.contains_key("candidates") {
        ret.insert(String::from("candidates"), serde_json::json!([]));
    }

    // Convert answer objects from a [...] to { "variants": [...] }.
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


fn print_correct() {
    println!("{}", "Correct!".green());
}


fn print_incorrect(answer: &str) {
    if answer.len() > 0 {
        let message = &format!(
            "{} The correct answer was {}.", "Incorrect.".red(), answer.green()
        );
        prettyprint(message);
    } else {
        println!("{}", "Incorrect.".red());
    }
}
