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
    if options.list_tags {
        list_tags(&quiz);
    } else if options.count {
        println!("{}", quiz.filter_questions(&options).len());
    } else {
        let results = quiz.take(&options);

        if results.len() > 0 && (options.save_results || yesno("\nSave results? ")) {
            let mut dirpath = dirs::data_dir().unwrap();
            dirpath.push("iafisher_popquiz");

            if !dirpath.as_path().exists() {
                let emsg = format!(
                    "Unable to create data directory at {}", dirpath.to_str().unwrap()
                );
                fs::create_dir(&dirpath).expect(&emsg);
            }

            dirpath.push("results.json");
            save_results(dirpath.to_str().unwrap(), &results);
        }
    }
}
