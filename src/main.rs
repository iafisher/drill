use popquiz::*;

fn main() {
    let quiz = Quiz {
        questions: vec![
            Box::new(ShortAnswerQuestion::new("What is the capital of Bulgaria?", "Sofia")),
            Box::new(ShortAnswerQuestion::new("What is the longest river in Asia?", "Yangtze")),
        ]
    };
    quiz.take();
}
