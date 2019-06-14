use std::fs;

use popquiz::*;


fn main() {
    let options = parse_options();

    if options.no_color {
        colored::control::set_override(false);
    }

    let mut master_list = Vec::new();
    for path in options.paths.iter() {
        let mut quiz: Quiz = load_quiz(path);
        master_list.append(&mut quiz.questions);
    }

    let mut quiz = Quiz { questions: master_list };
    if options.list_topics {
        list_topics(&quiz);
    } else if options.count {
        println!("{}", quiz.questions.len());
    } else {
        let results = quiz.take(&options);

        if options.save_results || yesno("\nSave results? ") {
            let mut dirpath = dirs::data_dir().unwrap();
            dirpath.push("iafisher_popquiz");

            if !dirpath.as_path().exists() {
                fs::create_dir(&dirpath)
                    .expect(&format!("Unable to create data directory at {}", dirpath.to_str().unwrap()));
            }

            dirpath.push("results.json");
            save_results(dirpath.to_str().unwrap(), &results);
        }
    }
}
