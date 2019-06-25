/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use colored::*;

use popquiz::*;


fn main() {
    let result = match parse_options() {
        QuizOptions::Take(options) => {
            main_take(options)
        },
        QuizOptions::Count(options) => {
            main_count(options)
        },
        QuizOptions::Results(options) => {
            main_results(options)
        },
        QuizOptions::Edit(options) => {
            main_edit(options)
        },
        QuizOptions::Delete(options) => {
            main_delete(options)
        },
        QuizOptions::List => {
            main_list()
        },
    };

    if let Err(e) = result {
        match e {
            QuizError::Json(e) => {
                show_error("could not parse JSON");
                eprintln!("  Reason: {}", e);
            }
            QuizError::QuizNotFound(name) => {
                show_error(&format!("no quiz named '{}' found", name));
            },
            QuizError::CannotMakeAppDir => {
                show_error("unable to create application directory");
            }
            QuizError::CannotOpenEditor => {
                show_error("system editor cannot be opened");
            }
        }
        ::std::process::exit(2);
    }
}


fn show_error(msg: &str) {
    eprintln!("{}: {}.", "Error".red(), msg);
}
