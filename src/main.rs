use popquiz::*;

fn main() {
    let quiz = Quiz {
        questions: vec![
            Question::new(
                QuestionKind::ShortAnswer, "What is the capital of Bulgaria?", "Sofia"
            ),
            Question::new(
                QuestionKind::ShortAnswer, "What is the longest river in Asia?", "Yangtze"
            ),
        ]
    };
    quiz.take();
    println!("{}", serde_json::to_string(&quiz).unwrap());
}
