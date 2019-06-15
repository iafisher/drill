/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use popquiz::*;


fn main() {
    // Parse command-line arguments.
    let options = parse_options();
    if options.no_color {
        colored::control::set_override(false);
    }

    // Consolidate the individual quiz files into a single `Quiz` object.
    let mut master_list = Vec::new();
    for path in options.paths.iter() {
        let mut quiz: Quiz = load_quiz(path);
        master_list.append(&mut quiz.questions);
    }
    let mut quiz = Quiz::new(master_list);

    // The main program.
    if options.list_tags {
        list_tags(&quiz);
    } else if options.count {
        println!("{}", quiz.filter_questions(&options).len());
    } else {
        let results = quiz.take(&options);
        if results.len() > 0 && (options.do_save_results || yesno("\nSave results? ")) {
            save_results(&results);
        }
    }
}
