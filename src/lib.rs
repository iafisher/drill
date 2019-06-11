use std::io;
use std::io::Write;

use chrono::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum QuestionKind {
    ShortAnswer, ListAnswer,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct QuestionResult {
    pub time_asked: DateTime<Utc>,
    pub result: bool,
}

impl<'a> Question<'a> {
    pub fn ask(&self) -> bool {
        println!("{}\n", self.text);

        match self.kind {
            QuestionKind::ShortAnswer => {
                let guess = prompt("> ");
                let result = self.check_any(&guess);
                if result {
                    println!("Correct!");
                } else {
                    println!("Incorrect!");
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

                return satisfied.iter().all(|x| *x);
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Quiz<'a> {
    #[serde(borrow)]
    pub questions: Vec<Question<'a>>,
}

impl<'a> Quiz<'a> {
    pub fn take(&mut self) -> Vec<(&Question, QuestionResult)> {
        let mut results = Vec::new();
        let mut total_correct = 0;
        let mut total = 0;
        for question in self.questions.iter() {
            println!("\n");
            let correct = question.ask();
            let result = QuestionResult {
                time_asked: Utc::now(),
                result: correct,
            };
            results.push((question, result));

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
