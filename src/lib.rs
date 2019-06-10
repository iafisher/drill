pub trait Question {
    fn ask(&self);
}

pub struct ShortAnswerQuestion<'a> {
    pub text: &'a str,
    pub answers: Vec<&'a str>,
}

impl<'a> Question for ShortAnswerQuestion<'a> {
    fn ask(&self) {
        println!("{}", self.text);
    }
}

pub struct Quiz {
    pub questions: Vec<Box<Question>>,
}

impl Quiz {
    pub fn take(&self) {
        for question in self.questions.iter() {
            question.ask();
        }
    }
}
