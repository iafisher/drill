use popquiz::*;

fn main() {
    let q = ShortAnswerQuestion {
        text: "What is the capital of Bulgaria?", answer: Answer { variants: vec!["Sofia"] }
    };
    let quiz = Quiz { questions: vec![Box::new(q)] };
    quiz.take();
}
