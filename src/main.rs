/**
 * Take a pop quiz from the command line.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: June 2019
 */
use popquiz::*;


fn main() {
    // Parse command-line arguments.
    match parse_options() {
        QuizOptions::Take(options) => {
            main_take(options);
        },
        QuizOptions::Count(options) => {
            main_count(options);
        },
        QuizOptions::Results(options) => {
            main_results(options);
        },
    }
}
