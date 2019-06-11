use std::fs;

use popquiz::*;


fn main() {
    let options = parse_options();

    let data = fs::read_to_string(&options.path)
        .expect("Unable to read from quiz file");
    let mut quiz: Quiz = serde_json::from_str(&data)
        .expect("Unable to deserialize JSON to Quiz object");

    if options.list_topics {
        list_topics(&quiz);
    } else {
        let results = quiz.take(&options);

        if options.save_results || yesno("\nSave results? ") {
            save_results(&derive_result_path(&options.path), &results);
        }
    }
}
