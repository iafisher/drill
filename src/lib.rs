extern crate argparse;
extern crate chrono;
extern crate rand;
extern crate textwrap;

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::Write;

use argparse::{ArgumentParser, Store, StoreTrue};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer, OrderedListAnswer,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Answer<'a> {
    #[serde(borrow)]
    pub variants: Vec<&'a str>,
}

impl<'a> Answer<'a> {
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
pub struct Question<'a> {
    pub kind: QuestionKind,
    pub text: &'a str,
    pub topic: &'a str,
    #[serde(borrow)]
    pub answers: Vec<Answer<'a>>,
}

impl<'a> Question<'a> {
    pub fn ask(&self) -> bool {
        prettyprint(&format!("{}\n", self.text));

        match self.kind {
            QuestionKind::ShortAnswer => {
                let guess = prompt("> ");
                let result = self.check_any(&guess);
                if result {
                    println!("Correct!");
                } else {
                    prettyprint(
                        &format!(
                            "Incorrect. The correct answer was {}.",
                            self.answers[0].variants[0]
                        )
                    );
                }
                return result;
            },
            QuestionKind::ListAnswer => {
                let mut satisfied = Vec::<bool>::with_capacity(self.answers.len());
                for _ in 0..self.answers.len() {
                    satisfied.push(false);
                }

                let mut count = 0;
                while count < self.answers.len() {
                    let guess = prompt("> ");
                    let index = self.check_one(&guess);
                    if index == self.answers.len() {
                        println!("Incorrect.");
                        count += 1;
                    } else if satisfied[index] {
                        println!("You already said that.");
                    } else {
                        satisfied[index] = true;
                        println!("Correct!");
                        count += 1;
                    }
                }

                let all_correct = satisfied.iter().all(|x| *x);
                if !all_correct {
                    println!("\nYou missed:");
                    for (i, correct) in satisfied.iter().enumerate() {
                        if !correct {
                            println!("  {}", self.answers[i].variants[0]);
                        }
                    }
                }
                return all_correct;
            }
            QuestionKind::OrderedListAnswer => {
                let mut correct = true;
                for answer in self.answers.iter() {
                    let guess = prompt("> ");
                    if answer.check(&guess) {
                        println!("Correct!");
                    } else {
                        prettyprint(
                            &format!(
                                "Incorrect. The correct answer was {}.",
                                answer.variants[0]
                            )
                        );
                        correct = false;
                    }
                }
                return correct;
            }
        }
    }

    fn check_any(&self, guess: &str) -> bool {
        for answer in self.answers.iter() {
            if answer.check(guess) {
                return true;
            }
        }
        false
    }

    fn check_one(&self, guess: &str) -> usize {
        for (i, answer) in self.answers.iter().enumerate() {
            if answer.check(guess) {
                return i;
            }
        }
        self.answers.len()
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuestionResult {
    pub time_asked: chrono::DateTime<chrono::Utc>,
    pub correct: bool,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Quiz<'a> {
    #[serde(borrow)]
    pub questions: Vec<Question<'a>>,
}

impl<'a> Quiz<'a> {
    pub fn take(&mut self, options: &QuizOptions) -> Vec<(&Question, QuestionResult)> {
        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total = 0;

        let questions = self.choose_questions(&options);
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
            println!("\n{} correct out of {} ({}%).", total_correct, total, score);
        }

        results
    }

    fn choose_questions(&self, options: &QuizOptions) -> Vec<&Question> {
        let mut rng = thread_rng();

        let mut candidates = Vec::new();
        for question in self.questions.iter() {
            if !self.filter_question(question, options) {
                candidates.push(question);
            }
        }

        candidates.shuffle(&mut rng);
        candidates.truncate(options.num_to_ask as usize);
        candidates
    }

    fn filter_question(&self, q: &Question, options: &QuizOptions) -> bool {
        options.topic.len() > 0 && q.topic != options.topic
    }
}


pub fn prompt(message: &str) -> String {
    print!("{}", message);
    io::stdout().flush()
        .expect("Unable to flush standard output");
    let mut response = String::new();
    io::stdin().read_line(&mut response)
        .expect("Failed to read line");

    // If the string is completely empty, then the user hit Ctrl+D and we should exit.
    // A blank line is indicated by "\n" rather than "".
    if response.len() == 0 {
        println!("");
        ::std::process::exit(2);
    }

    response.trim_end().to_string()
}


pub fn prettyprint(message: &str) {
    println!("{}", textwrap::fill(message, textwrap::termwidth()));
}


pub fn yesno(message: &str) -> bool {
    let response = prompt(message);
    response.trim_start().to_lowercase().starts_with("y")
}


pub struct QuizOptions {
    pub topic: String,
    pub num_to_ask: u16,
    pub list_topics: bool,
    pub save_results: bool,
}


pub fn parse_options() -> QuizOptions {
    let mut topic = String::new();
    let mut num_to_ask = 10;
    let mut list_topics = false;
    let mut save_results = false;
    {
        let mut parser = ArgumentParser::new();
        parser.set_description("Take a pop quiz from the command line.");

        parser.refer(&mut topic)
            .add_option(&["--topic"], Store, "Restrict questions to a certain topic.");

        parser.refer(&mut num_to_ask)
            .add_option(&["-n"], Store, "Number of questions to ask.");

        parser.refer(&mut list_topics)
            .add_option(&["--list-topics"], StoreTrue, "List all available topics.");

        parser.refer(&mut save_results)
            .add_option(&["--save"], StoreTrue, "Save quiz results without prompting.");

        parser.parse_args_or_exit();
    }
    QuizOptions { topic, num_to_ask, list_topics, save_results }
}


pub fn list_topics(quiz: &Quiz) {
    let mut topics = HashSet::new();
    for question in quiz.questions.iter() {
        if question.topic.len() > 0 {
            topics.insert(question.topic);
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
        if !hash.contains_key(q.text) {
            hash.insert(q.text, Vec::new());
        }
        hash.get_mut(q.text).unwrap().push((*qr).clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .expect("Unable to serialize results object to JSON");
    fs::write(path, serialized_results)
        .expect("Unable to write to quiz file");
    println!("Results saved to {}.", path);
}
