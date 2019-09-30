/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: September 2019
 */
use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;

use serde::Deserialize;
use toml;
use toml::Value;

use super::quiz;


type AnswerV2 = Vec<String>;


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
    let mut reader = BufReader::new(File::open(path).unwrap());
    let mut questions = Vec::new();
    loop {
        if let Some(entry) = read_entry(&mut reader) {
            let q = entry_to_question(&entry);
            questions.push(q);
        } else {
            break;
        }
    }
    questions
}

fn entry_to_question(entry: &FileEntry) -> QuestionWrapper {
    // TODO: Handle multiple question texts.
    let q = if entry.following.len() == 1 {
        if let Some(choices) = entry.attributes.get("choices") {
            QuestionV2::MultipleChoice {
                text: vec![entry.text.clone()],
                answer: split(&entry.following[0], "/"),
                choices: split(&choices, "/"),
            }
        } else {
            QuestionV2::ShortAnswer {
                text: vec![entry.text.clone()],
                answer: split(&entry.following[0], "/"),
            }
        }
    } else if entry.following.len() == 0 {
        // TODO: Handle case where there is no '='.
        let equal = entry.text.find("=").unwrap();
        let top = entry.text[..equal].trim().to_string();
        let bottom = split(&entry.text[equal+1..], "/");
        QuestionV2::Flashcard { top, bottom }
    } else {
        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            // TODO: Error if not in correct format.
            _ordered == "true"
        } else {
            false
        };
        QuestionV2::List {
            text: vec![entry.text.clone()],
            answers: entry.following.iter().map(|l| split(&l, "/")).collect(),
            ordered,
        }
    };
    // TODO: Parse tags.
    let tags = entry.attributes.get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());
    let w = QuestionWrapper { id: entry.id.clone(), question: q, tags };
    println!("{:?}", w);
    w
}

fn read_entry(reader: &mut BufReader<File>) -> Option<FileEntry> {
    match read_line(reader) {
        Some(FileLine::First(id, text)) => {
            let mut entry = FileEntry {
                id, text, following: Vec::new(), attributes: HashMap::new(),
            };
            loop {
                match read_line(reader) {
                    Some(FileLine::Blank) | None => {
                        break;
                    },
                    Some(FileLine::Following(line)) => {
                        entry.following.push(line);
                    },
                    Some(FileLine::Pair(key, value)) => {
                        entry.attributes.insert(key, value);
                    },
                    Some(FileLine::First(..)) => {
                        // TODO: Return an error.
                    }
                }
            }
            Some(entry)
        },
        Some(_) => {
            // TODO: Return an error.
            None
        },
        None => {
            None
        },
    }
}

fn read_line(reader: &mut BufReader<File>) -> Option<FileLine> {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap() == 0 {
        return None;
    }

    let trimmed = line.trim();
    if trimmed.starts_with("#") {
        read_line(reader)
    } else if trimmed.len() == 0 {
        Some(FileLine::Blank)
    } else if trimmed.starts_with("- ") {
        let colon = trimmed.find(":").unwrap();
        let key = trimmed[2..colon].trim().to_string();
        let value = trimmed[colon+1..].trim().to_string();
        Some(FileLine::Pair(key, value))
    } else if trimmed.starts_with("[") && trimmed.find("]").is_some() {
        let brace = trimmed.find("]").unwrap();
        let id = &trimmed[1..brace];
        let rest = &trimmed[brace+1..];
        Some(FileLine::First(id.trim().to_string(), rest.trim().to_string()))
    } else {
        Some(FileLine::Following(trimmed.to_string()))
    }
}


fn split(s: &str, splitter: &str) -> Vec<String> {
    s.split(splitter).map(|w| w.trim().to_string()).collect()
}


enum FileLine {
    First(String, String),
    Following(String),
    Pair(String, String),
    Blank,
}

struct FileEntry {
    id: String,
    text: String,
    following: Vec<String>,
    attributes: HashMap<String, String>,
}
