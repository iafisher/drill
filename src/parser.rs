/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf; 
use std::str::FromStr;
use super::common::{Location, QuizError};
use super::quiz::{Question, QuestionKind, Quiz};


pub fn parse(path: &PathBuf) -> Result<Quiz, QuizError> {
    let file = File::open(path).map_err(QuizError::Io)?;
    let mut reader = QuizReader::new(BufReader::new(file));
    let quiz_settings = read_settings(&mut reader)?;

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

    for mut q in questions.iter_mut() {
        apply_global_settings(&quiz_settings, &mut q);
    }

    Ok(Quiz { instructions: quiz_settings.instructions, questions })
}

fn entry_to_question(entry: &FileEntry) -> Result<Question, QuizError> {
    let tags = entry.attributes.get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());

    let timeout = if let Some(_timeout) = entry.attributes.get("timeout") {
        Some(parse_u64(_timeout, entry.location.line)?)
    } else {
        None
    };

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
                timeout,
                front_context: None,
                back_context: None,
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
                timeout,
                front_context: None,
                back_context: None,
            });
        }
    } else if entry.following.len() == 0 {
        if let Some(equal) = entry.text.find("=") {
            let frnt = entry.text[..equal].trim().to_string();
            let (front, front_context) = get_context(&frnt, entry.location.line)?;
            let bck = &entry.text[equal+1..];
            let (back, back_context) = get_context(&bck, entry.location.line)?;
            let back_split = split(&back, "/");
            return Ok(Question {
                kind: QuestionKind::Flashcard,
                id: entry.id.clone(),
                text: vec![front],
                answer_list: vec![back_split],
                candidates: Vec::new(),
                no_credit: Vec::new(),
                prior_results: Vec::new(),
                tags,
                explanations: Vec::new(),
                location: Some(entry.location.clone()),
                timeout,
                front_context,
                back_context,
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
                timeout: None,
                front_context: None,
                back_context: None,
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
                timeout: None,
                front_context: None,
                back_context: None,
            });
        }
    }
}

#[derive(Debug)]
struct GlobalSettings {
    instructions: Option<String>,
    timeout: Option<u64>,
}


fn apply_global_settings(settings: &GlobalSettings, question: &mut Question) {
    if let Some(timeout) = settings.timeout {
        if question.timeout.is_none()
           && question.kind != QuestionKind::ListAnswer
           && question.kind != QuestionKind::OrderedListAnswer {
            question.timeout.replace(timeout);
        }
    }
}


/// Read the initial settings from the file.
fn read_settings(reader: &mut QuizReader) -> Result<GlobalSettings, QuizError> {
    let mut settings = GlobalSettings { instructions: None, timeout: None };
    let mut first_line = true;
    loop {
        match reader.read_line()? {
            Some(FileLine::Pair(key, val)) => {
                if key == "instructions" {
                    settings.instructions.replace(val);
                } else if key == "timeout" {
                    settings.timeout.replace(parse_u64(&val, reader.line)?);
                }
            },
            Some(FileLine::Blank) | None => {
                break;
            },
            Some(line) => {
                // An unexpected line type is okay for the first line: it just means
                // that the quiz doesn't have any settings. But it's not okay after,
                // because the settings must be separated from the rest of the quiz by
                // a blank line.
                if first_line {
                    reader.push(line);
                    break;
                } else {
                    return Err(QuizError::Parse { line: reader.line, whole_entry: false });
                }
            },
        }
        first_line = false;
    }
    Ok(settings)
}


/// Read an entry from the file.
///
/// `Ok(Some(entry))` is returned on a successful read. `Ok(None)` is returned when the
/// end of file is reached. `Err(e)` is returned if a parse error occurs.
fn read_entry(path: &PathBuf, reader: &mut QuizReader) -> Result<Option<FileEntry>, QuizError> {
    match reader.read_line()? {
        Some(FileLine::First(id, text)) => {
            let mut entry = FileEntry {
                id,
                text,
                following: Vec::new(),
                attributes: HashMap::new(),
                location: Location { line: reader.line, path: path.clone() },
            };
            loop {
                match reader.read_line()? {
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


struct QuizReader {
    reader: BufReader<File>,
    /// This field is for when a function reads one line too many and needs to "push" it
    /// back so that the next call to `read_line` returns it instead of the next line
    /// in the underlying file.
    pushed: Option<FileLine>,
    line: usize,
}


impl QuizReader {
    pub fn new(reader: BufReader<File>) -> Self {
        Self { reader, pushed: None, line: 0 }
    }

    /// Push a line so that the next time `read_line` is called it returns `line`
    /// instead of the next line from the file.
    ///
    /// Only one line can be buffered at a time; if you call `push` when a line is
    /// already buffered, the old line will be replaced.
    pub fn push(self: &mut QuizReader, line: FileLine) {
        self.pushed.replace(line);
    }

    /// Read a line from the underlying file. Possible return values:
    ///   - `Ok(Some(_))`: a line was successfully read
    ///   - `Ok(None)`: end of file
    ///   - `Err(_)`: the read did not succeed (e.g., I/O error, parse error)
    pub fn read_line(self: &mut QuizReader) -> Result<Option<FileLine>, QuizError> {
        if self.pushed.is_some() {
            return Ok(self.pushed.take());
        }

        let mut line = String::new();
        let nread = self.reader.read_line(&mut line).map_err(QuizError::Io)?;
        if nread == 0 {
            return Ok(None);
        }
        self.line += 1;

        let trimmed = line.trim();
        if trimmed.starts_with("#") {
            self.read_line()
        } else if trimmed.len() == 0 {
            Ok(Some(FileLine::Blank))
        } else if trimmed.starts_with("- ") {
            if let Some(colon) = trimmed.find(":") {
                let key = trimmed[2..colon].trim().to_string();
                let value = trimmed[colon+1..].trim().to_string();
                Ok(Some(FileLine::Pair(key, value)))
            } else {
                Err(QuizError::Parse { line: self.line, whole_entry: false })
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
}


fn parse_u64(s: &str, line: usize) -> Result<u64, QuizError> {
    u64::from_str(s).map_err(|_| QuizError::Parse { line, whole_entry: false })
}


fn get_context(line: &str, lineno: usize) -> Result<(String, Option<String>), QuizError> {
    if let Some(open) = line.find('[') {
        if let Some(_close) = line[open..].find(']') {
            let close = _close + open;
            let new_line = String::from(line[..open].trim());
            let context = String::from(line[open+1..close].trim());
            Ok((new_line, Some(context)))
        } else {
            Err(QuizError::Parse { line: lineno, whole_entry: false })
        }
    } else {
        Ok((String::from(line), None))
    }
}
