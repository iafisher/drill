/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: July 2019
 */
use colored::*;

use popquiz::*;


fn main() {
    let options = parse_options();

    let result = match options {
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
        QuizOptions::Rm(options) => {
            main_rm(options)
        },
        QuizOptions::Mv(options) => {
            main_mv(options)
        },
        QuizOptions::Ls(options) => {
            main_ls(options)
        },
        QuizOptions::Path(options) => {
            main_path(options)
        },
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}
