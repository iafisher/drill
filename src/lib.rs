use std::io;
use std::io::Write;

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
    #[serde(borrow)]
    pub answers: Vec<Answer<'a>>,

}

impl<'a> Question<'a> {
    pub fn short_answer(text: &'a str, answer: &'a str) -> Self {
        Self {
            kind: QuestionKind::ShortAnswer,
            text,
            answers: vec![Answer { variants: vec![answer] }]
        }
    }

    pub fn short_answer_multiple(text: &'a str, variants: &[&'a str]) -> Self {
        let mut answers = Vec::<Answer>::new();
        for variant in variants.iter() {
            answers.push(Answer { variants: vec![variant] });
        }
        Self {
            kind: QuestionKind::ShortAnswer, text, answers
        }
    }

    pub fn ask(&self) -> bool {
        println!("{}\n", self.text);

        match self.kind {
            QuestionKind::ShortAnswer => {
                let guess = self.ask_once();
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
                    let guess = self.ask_once();
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

    fn ask_once(&self) -> String {
        print!("> ");
        io::stdout().flush()
            .expect("Unable to flush standard output");
        let mut guess = String::new();
        io::stdin().read_line(&mut guess)
            .expect("Failed to read line");
        guess.trim_end().to_string()
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
    pub fn take(&self) {
        let mut total_correct = 0;
        let mut total = 0;
        for question in self.questions.iter() {
            println!("\n");
            let correct = question.ask();

            total += 1;
            if correct {
                total_correct += 1;
            }
        }

        if total > 0 {
            let score = (total_correct as f64) / (total as f64) * 100.0;
            println!("\n{} correct out of {} ({}%).", total_correct, total, score);
        }
    }
}
