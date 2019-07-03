/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: July 2019
 */
use std::io;

use colored::*;

use popquiz::*;


fn foo<W: io::Write>(writer: &mut W) {
    write!(writer, "> ");
    let mut reader = rustyline::Editor::<()>::new();
    reader.readline("");
}


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
            main_delete(&mut writer, &mut reader, options)
        },
        QuizOptions::List => {
            main_list(&mut writer)
        },
    };

    if let Err(e) = result {
        if !is_broken_pipe(&e) {
            eprintln!("{}: {}", "Error".red(), e);
            ::std::process::exit(2);
        }
    }
}
