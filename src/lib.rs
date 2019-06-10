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
            if *variant == guess {
                return true;
            }
        }
        false
    }
}

pub struct ShortAnswerQuestion<'a> {
    pub text: &'a str,
    pub answer: Answer<'a>,
}

impl<'a> Question for ShortAnswerQuestion<'a> {
    fn ask(&self) -> bool {
        println!("{}\n", self.text);

        print!("> ");
        io::stdout().flush();
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
            println!("\n{} correct out of {} ({}%).", total, total_correct, score);
        }
    }
}
