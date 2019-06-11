use std::fs;

use popquiz::*;


const QUIZ_PATH: &str = "/home/iafisher/dev/popquiz/quiz.json";
const RESULTS_PATH: &str = "/home/iafisher/dev/popquiz/quiz_results.json";


fn main() {
    let options = parse_options();

    let data = fs::read_to_string(QUIZ_PATH)
        .expect("Unable to read from quiz file");
    let mut quiz: Quiz = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON to Quiz object");

    if options.list_topics {
        list_topics(&quiz);
    } else {
        let results = quiz.take(&options);

        if yesno("\nSave results? ") {
            save_results(RESULTS_PATH, &results);
        }
    }
}
