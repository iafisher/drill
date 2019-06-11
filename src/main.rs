use popquiz::*;

fn main() {
    let quiz = Quiz {
        questions: vec![
            Question::short_answer("What is the capital of Bulgaria?", "Sofia"),
            Question::short_answer_multiple(
                "What is the longest river in Asia?", &["Yangtze", "Yangtze River"]
            ),
        ]
    };
    quiz.take();
    println!("{}", serde_json::to_string(&quiz).unwrap());
}
