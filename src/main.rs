use std::fs;

use popquiz::*;

fn main() {
    let data = fs::read_to_string("/home/iafisher/dev/popquiz/quiz.json")
        .expect("Unable to read from quiz file");
    let quiz: Quiz = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON");
    quiz.take();
}
