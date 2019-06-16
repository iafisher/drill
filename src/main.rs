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
        match load_quiz(path) {
            Ok(mut quiz) => {
                master_list.append(&mut quiz.questions);
            },
            Err(e) => {
                eprintln!("Error on {}: {}", path, e);
                ::std::process::exit(2);
            }
        }
    }
    let mut quiz = Quiz::new(master_list);

    // The main program.
    if options.list_tags {
        list_tags(&quiz);
    } else if options.count {
        println!("{}", quiz.filter_questions(&options).len());
    } else if options.delete_results || options.force_delete_results {
        let prompt = "Are you sure you want to delete all previous results? ";
        if options.force_delete_results || yesno(&prompt) {
            delete_results();
        }
    } else if options.print_results {
        print_results();
    } else {
        let results = quiz.take(&options);
        if results.len() > 0 && (options.do_save_results || yesno("\nSave results? ")) {
            save_results(&results);
        }
    }
}
