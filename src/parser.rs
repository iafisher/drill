/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;

use super::common::{Location, QuizError};
use super::quiz::{Question, QuestionKind, Quiz};


pub fn parse(path: &PathBuf) -> Result<Quiz, QuizError> {
    let file = File::open(path).map_err(QuizError::Io)?;
    let mut reader = LineBufReader { reader: BufReader::new(file), line: 0 };
    let mut questions = Vec::new();
    loop {
        match read_entry(&path, &mut reader) {
            Ok(Some(entry)) => {
                let q = entry_to_question(&entry)?;
                questions.push(q);
            },
            Ok(None) => {
                break;
            },
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(Quiz { instructions: None, questions })
}

fn entry_to_question(entry: &FileEntry) -> Result<Question, QuizError> {
    let tags = entry.attributes.get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());

    // TODO: Handle multiple question texts.
    if entry.following.len() == 1 {
        if let Some(choices) = entry.attributes.get("choices") {
            return Ok(Question {
                kind: QuestionKind::MultipleChoice,
                id: entry.id.clone(),
                text: vec![entry.text.clone()],
                answer_list: vec![split(&entry.following[0], "/")],
                candidates: split(&choices, "/"),
                no_credit: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
            });
        } else {
            return Ok(Question {
                kind: QuestionKind::ShortAnswer,
                id: entry.id.clone(),
                text: vec![entry.text.clone()],
                answer_list: vec![split(&entry.following[0], "/")],
                candidates: Vec::new(),
                no_credit: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
            });
        }
    } else if entry.following.len() == 0 {
        if let Some(equal) = entry.text.find("=") {
            let top = entry.text[..equal].trim().to_string();
            let bottom = split(&entry.text[equal+1..], "/");
            return Ok(Question {
                kind: QuestionKind::Flashcard,
                id: entry.id.clone(),
                text: vec![top],
                answer_list: vec![bottom],
                candidates: Vec::new(),
                no_credit: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
            });
        } else {
            return Err(QuizError::Parse { line: entry.location.line, whole_entry: true });
        }
    } else {
        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            if _ordered != "true" && _ordered != "false" {
                return Err(QuizError::Parse { line: entry.location.line, whole_entry: true });
            }
            _ordered == "true"
        } else {
            false
        };

        let no_credit = if let Some(_no_credit) = entry.attributes.get("nocredit") {
            split(&_no_credit, "/")
        } else {
            Vec::new()
        };

        if ordered {
            return Ok(Question {
                kind: QuestionKind::OrderedListAnswer,
                id: entry.id.clone(),
                text: vec![entry.text.clone()],
                answer_list: entry.following.iter().map(|l| split(&l, "/")).collect(),
                candidates: Vec::new(),
                no_credit,
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
            });
        } else {
            return Ok(Question {
                kind: QuestionKind::ListAnswer,
                id: entry.id.clone(),
                text: vec![entry.text.clone()],
                answer_list: entry.following.iter().map(|l| split(&l, "/")).collect(),
                candidates: Vec::new(),
                no_credit,
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
            });
        }
    }
}


/// Read an entry from the file.
///
/// `Ok(Some(entry))` is returned on a successful read. `Ok(None)` is returned when the
/// end of file is reached. `Err(e)` is returned if a parse error occurs.
fn read_entry(path: &PathBuf, reader: &mut LineBufReader) -> Result<Option<FileEntry>, QuizError> {
    match read_line(reader)? {
        Some(FileLine::First(id, text)) => {
            let mut entry = FileEntry {
                id,
                text,
                following: Vec::new(),
                attributes: HashMap::new(),
                location: Location { line: reader.line, path: path.clone() },
            };
            loop {
                match read_line(reader)? {
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
                        return Err(
                            QuizError::Parse { line: reader.line, whole_entry: false }
                        );
                    }
                }
            }
            Ok(Some(entry))
        },
        Some(_) => {
            Err(QuizError::Parse { line: reader.line, whole_entry: false })
        },
        None => {
            Ok(None)
        },
    }
}

fn read_line(reader: &mut LineBufReader) -> Result<Option<FileLine>, QuizError> {
    let mut line = String::new();
    let nread = reader.read_line(&mut line).map_err(QuizError::Io)?;
    if nread == 0 {
        return Ok(None);
    }

    let trimmed = line.trim();
    if trimmed.starts_with("#") {
        read_line(reader)
    } else if trimmed.len() == 0 {
        Ok(Some(FileLine::Blank))
    } else if trimmed.starts_with("- ") {
        if let Some(colon) = trimmed.find(":") {
            let key = trimmed[2..colon].trim().to_string();
            let value = trimmed[colon+1..].trim().to_string();
            Ok(Some(FileLine::Pair(key, value)))
        } else {
            Err(QuizError::Parse { line: reader.line, whole_entry: false })
        }
    } else if trimmed.starts_with("[") && trimmed.find("]").is_some() {
        let brace = trimmed.find("]").unwrap();
        let id = &trimmed[1..brace];
        let rest = &trimmed[brace+1..];
        Ok(Some(FileLine::First(id.trim().to_string(), rest.trim().to_string())))
    } else {
        Ok(Some(FileLine::Following(trimmed.to_string())))
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
    location: Location,
}


struct LineBufReader {
    reader: BufReader<File>,
    line: usize,
}


impl LineBufReader {
    pub fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        self.line += 1;
        self.reader.read_line(buf)
    }
}
