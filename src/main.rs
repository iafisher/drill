/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: July 2019
 */
use std::io;

use colored::*;

use popquiz::*;


fn main() {
    let options = parse_options();
    let mut reader = rustyline::Editor::<()>::new();
    let mut writer = io::stdout();

    let result = match options {
        QuizOptions::Take(options) => {
            main_take(&mut writer, &mut reader, options)
        },
        QuizOptions::Count(options) => {
            main_count(options)
        },
        QuizOptions::Results(options) => {
            main_results(&mut writer, options)
        },
        QuizOptions::Edit(options) => {
            main_edit(options)
        },
        QuizOptions::Delete(options) => {
            main_delete(&mut writer, &mut reader, options)
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
