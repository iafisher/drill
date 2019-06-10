use std::io;
use std::io::Write;

pub trait Question {
    fn ask(&self) -> bool;
}

pub struct Answer<'a> {
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

pub struct ShortAnswerQuestion<'a> {
    text: &'a str,
    answer: Answer<'a>,
}

impl<'a> ShortAnswerQuestion<'a> {
    pub fn new(text: &'a str, answer: &'a str) -> Self {
        Self { text, answer: Answer { variants: vec![answer] } }
    }
}

impl<'a> Question for ShortAnswerQuestion<'a> {
    fn ask(&self) -> bool {
        println!("{}\n", self.text);

        print!("> ");
        io::stdout().flush()
            .expect("Unable to flush standard output");
        let mut guess = String::new();
        io::stdin().read_line(&mut guess)
            .expect("Failed to read line");

        self.answer.check(&guess.trim_end())
    }
}

pub struct Quiz {
    pub questions: Vec<Box<Question>>,
}

impl Quiz {
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
