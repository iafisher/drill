/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: September 2019
 */
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use toml;
use toml::Value;

use super::quiz;


type AnswerV2 = Vec<String>;


#[derive(Deserialize)]
struct FlatQuestion {
    q: Option<Vec<String>>,
    a: Option<AnswerV2>,
    answers: Option<Vec<AnswerV2>>,
    choices: Option<Vec<String>>,
    t: Option<String>,
    b: Option<AnswerV2>,
    ordered: Option<bool>,
    tags: Option<Vec<String>>,
}


#[derive(Debug)]
pub enum QuestionV2 {
    ShortAnswer { text: Vec<String>, answer: AnswerV2 },
    Flashcard { top: String, bottom: AnswerV2 },
    List { text: Vec<String>, answers: Vec<AnswerV2>, ordered: bool },
    MultipleChoice { text: Vec<String>, answer: AnswerV2, choices: Vec<String> },
}


#[derive(Debug)]
pub struct QuestionWrapper {
    id: String,
    question: QuestionV2,
    tags: Vec<String>,
}


pub fn parse(path: &PathBuf) -> Vec<QuestionWrapper> {
    let contents = fs::read_to_string(path).unwrap();
    let toml_value: HashMap<String, FlatQuestion> = toml::from_str(&contents).unwrap();

    let mut questions = Vec::new();
    for (id, toml_question) in toml_value.iter() {
        let q = if let Some(text) = &toml_question.q {
            if let Some(a) = &toml_question.a {
                if let Some(choices) = &toml_question.choices {
                    QuestionV2::MultipleChoice {
                        text: text.clone(),
                        answer: a.clone(),
                        choices: choices.clone()
                    }
                } else {
                    QuestionV2::ShortAnswer { text: text.clone(), answer: a.clone() }
                }
            } else {
                QuestionV2::List {
                    text: text.clone(),
                    answers: toml_question.answers.clone().unwrap(),
                    ordered: toml_question.ordered.unwrap_or(false)
                }
            }
        } else if let Some(top) = &toml_question.t {
            QuestionV2::Flashcard {
                top: top.clone(),
                bottom: toml_question.b.clone().unwrap(),
            }
        } else {
            panic!();
        };

        let wrapper = QuestionWrapper {
            id: id.to_string(),
            question: q,
            tags: toml_question.tags.clone().unwrap_or(Vec::new()),
        };
        questions.push(wrapper);
    }
    questions
}
