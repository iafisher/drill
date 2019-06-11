use std::io;
use std::io::Write;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum QuestionKind {
    ShortAnswer,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Question<'a> {
    pub kind: QuestionKind,
    pub text: &'a str,
    pub answers: Vec<&'a str>,

}

impl<'a> Question<'a> {
    pub fn new(kind: QuestionKind, text: &'a str, answer: &'a str) -> Self {
        Self { kind, text, answers: vec![answer] }
    }

    pub fn ask(&self) -> bool {
        println!("{}\n", self.text);

        print!("> ");
        io::stdout().flush()
            .expect("Unable to flush standard output");
        let mut guess = String::new();
        io::stdin().read_line(&mut guess)
            .expect("Failed to read line");

        self.check(&guess.trim_end())
    }

    fn check(&self, guess: &str) -> bool {
        for answer in self.answers.iter() {
            if answer.to_lowercase() == guess.to_lowercase() {
                return true;
            }
        }
        false
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Quiz<'a> {
    #[serde(borrow)]
    pub questions: Vec<Question<'a>>,
}

impl<'a> Quiz<'a> {
    pub fn take(&self) {
        let mut total_correct = 0;
        let mut total = 0;
        for question in self.questions.iter() {
            println!("\n");
            let correct = question.ask();

            total += 1;
            if correct {
                println!("\nCorrect!");
                total_correct += 1;
            } else {
                println!("\nIncorrect.");
            }
        }

        if total > 0 {
            let score = (total_correct as f64) / (total as f64) * 100.0;
            println!("\n{} correct out of {} ({}%).", total_correct, total, score);
        }
    }
}
