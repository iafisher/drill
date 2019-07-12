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
            main_count(&mut writer, options)
        },
        QuizOptions::Results(options) => {
            main_results(&mut writer, options)
        },
        QuizOptions::Edit(options) => {
            main_edit(options)
        },
        QuizOptions::Delete(options) => {
            main_delete(&mut reader, options)
        },
        QuizOptions::Rename(options) => {
            main_rename(options)
        },
        QuizOptions::List(options) => {
            main_list(&mut writer, options)
        },
        QuizOptions::Path(options) => {
            main_path(&mut writer, options)
        },
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}
