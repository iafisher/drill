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

use super::quiz::{Answer, Question, QuestionKind, Quiz};


pub fn parse(path: &PathBuf) -> Quiz {
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
    Quiz { default_kind: None, instructions: None, questions }
}

fn entry_to_question(entry: &FileEntry) -> Question {
    let tags = entry.attributes.get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());

    // TODO: Handle multiple question texts.
    if entry.following.len() == 1 {
        if let Some(choices) = entry.attributes.get("choices") {
            Question {
                kind: QuestionKind::MultipleChoice,
                text: vec![entry.text.clone()],
                answer_list: vec![split_to_answer(&entry.following[0], "/")],
                candidates: split(&choices, "/"),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
            }
        } else {
            Question {
                kind: QuestionKind::ShortAnswer,
                text: vec![entry.text.clone()],
                answer_list: vec![split_to_answer(&entry.following[0], "/")],
                candidates: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
            }
        }
    } else if entry.following.len() == 0 {
        // TODO: Handle case where there is no '='.
        let equal = entry.text.find("=").unwrap();
        let top = entry.text[..equal].trim().to_string();
        let bottom = split_to_answer(&entry.text[equal+1..], "/");
        Question {
            kind: QuestionKind::Flashcard,
            text: vec![top],
            answer_list: vec![bottom],
            candidates: Vec::new(),
            prior_results: Vec::new(),
            tags,
            explanations: Vec::new(),
        }
    } else {
        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            // TODO: Error if not in correct format.
            _ordered == "true"
        } else {
            false
        };

        if ordered {
            Question {
                kind: QuestionKind::OrderedListAnswer,
                text: vec![entry.text.clone()],
                answer_list: entry.following.iter().map(|l| split_to_answer(&l, "/")).collect(),
                candidates: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
            }
        } else {
            Question {
                kind: QuestionKind::ListAnswer,
                text: vec![entry.text.clone()],
                answer_list: entry.following.iter().map(|l| split_to_answer(&l, "/")).collect(),
                candidates: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
            }
        }
    }
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


fn split_to_answer(s: &str, splitter: &str) -> Answer {
    Answer { variants: split(s, splitter) }
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
