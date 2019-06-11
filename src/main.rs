use std::fs;

use popquiz::*;

const QUIZ_PATH: &str = "/home/iafisher/dev/popquiz/quiz.json";
const RESULTS_PATH: &str = "/home/iafisher/dev/quiz_results.json";

fn main() {
    let data = fs::read_to_string(QUIZ_PATH)
        .expect("Unable to read from quiz file");
    let mut quiz: Quiz = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON to Quiz object");

    let results = quiz.take();

    let yesno = prompt("\nSave results? ");
    if yesno.to_lowercase().starts_with("y") {
        let serialized_results = serde_json::to_string_pretty(&results)
            .expect("Unable to serialize results object to JSON");
        fs::write(RESULTS_PATH, serialized_results)
            .expect("Unable to write to quiz file");
        println!("Results saved to {}.", RESULTS_PATH);
    }
}
