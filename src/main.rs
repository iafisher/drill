/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: July 2019
 */
use colored::*;

use popquiz::*;


fn main() {
    // Exit quietly on broken pipe error.
    // Courtesy of https://github.com/rust-lang/rust/issues/46016#issuecomment-428106774
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

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
        eprintln!("{}: {}", "Error".red(), e);
        ::std::process::exit(2);
    }
}
