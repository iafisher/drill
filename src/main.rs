use popquiz::*;

fn main() {
    let q = ShortAnswerQuestion {
        text: "What is the capital of Bulgaria?", answers: vec!["Sofia"]
    };
    let quiz = Quiz { questions: vec![Box::new(q)] };
    quiz.take();
}
