/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: September 2019
 */
use super::quiz;


pub fn parse(path: &str) -> quiz::Quiz {
    quiz::Quiz { default_kind: None, instructions: None, questions: vec![] }
}
