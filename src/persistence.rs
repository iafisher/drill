/**
 * Functions and data structures for reading and writing quiz and results files in the
 * filesystem.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: October 2019
 */
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::common::{Location, QuizError, Result};
use super::quiz::{
    FlashcardQuestion, ListQuestion, MultipleChoiceQuestion, OrderedListQuestion,
    Question, QuestionCommon, QuestionResult, Quiz, QuizResult, ShortAnswerQuestion};


/// Load a `Quiz` object given its name.
pub fn load_quiz(dir: &Path, name: &str) -> Result<Quiz> {
    let old_results = load_results(&dir, name)?;

    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push(name);
    parse(&dir_mutable, &old_results)
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


pub fn load_results(dir: &Path, name: &str) -> Result<StoredResults> {
    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push("results");
    dir_mutable.push(format!("{}_results.json", name));
    match fs::read_to_string(dir_mutable) {
        Ok(data) => {
            serde_json::from_str(&data).map_err(QuizError::Json)
        },
        Err(_) => {
            Ok(HashMap::new())
        }
    }
}


/// Save `results` to a file in the popquiz application's data directory, appending the
/// results if previous results have been saved.
pub fn save_results(dir: &Path, name: &str, results: &QuizResult) -> Result<()> {
    let mut dir_mutable = dir.to_path_buf();
    dir_mutable.push("results");
    if !dir_mutable.as_path().exists() {
        fs::create_dir(&dir_mutable).map_err(QuizError::Io)?;
    }

    // Load old data, if it exists.
    dir_mutable.push(format!("{}_results.json", name));
    let data = fs::read_to_string(&dir_mutable);
    let mut hash: BTreeMap<String, Vec<QuestionResult>> = match data {
        Ok(ref data) => {
            serde_json::from_str(&data)
                .map_err(QuizError::Json)?
        },
        Err(_) => {
            BTreeMap::new()
        }
    };

    // Store the results as a map from the text of the questions to a list of individual
    // time-stamped results.
    for result in results.per_question.iter() {
        if !hash.contains_key(&result.id) {
            hash.insert(result.id.to_string(), Vec::new());
        }
        hash.get_mut(&result.id).unwrap().push(result.clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash)
        .map_err(QuizError::Json)?;
    fs::write(&dir_mutable, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(dir_mutable.clone())))?;
    Ok(())
}


fn parse(path: &PathBuf, old_results: &StoredResults) -> Result<Quiz> {
    let file = File::open(path).map_err(QuizError::Io)?;
    let mut reader = QuizReader::new(BufReader::new(file));
    let quiz_settings = read_settings(&mut reader)?;

    let mut questions = Vec::new();
    loop {
        match read_entry(&path, &mut reader) {
            Ok(Some(entry)) => {
                let q = entry_to_question(&entry, &quiz_settings, old_results)?;
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

    Ok(Quiz { instructions: quiz_settings.instructions, questions })
}

fn entry_to_question(
    entry: &FileEntry,
    settings: &GlobalSettings,
    old_results: &StoredResults) -> Result<Box<Question>> {

    let tags = entry.attributes.get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());

    let prior_results = old_results.get(&entry.id)
        .map(|v| v.clone())
        .unwrap_or(Vec::new());

    let common = QuestionCommon {
        id: entry.id.clone(),
        prior_results,
        tags,
        location: Some(entry.location.clone()),
    };

    let timeout = if let Some(_timeout) = entry.attributes.get("timeout") {
        Some(parse_u64(_timeout, entry.location.line)?)
    } else {
        settings.timeout
    };

    // TODO: Handle multiple question texts.
    let text = entry.text.clone();
    if entry.following.len() == 1 {
        let answer = split(&entry.following[0], "/");
        if let Some(choices) = entry.attributes.get("choices") {
            return Ok(Box::new(MultipleChoiceQuestion {
                text, answer, choices: split(&choices, "/"), timeout, common
            }));
        } else {
            return Ok(Box::new(ShortAnswerQuestion { text, answer, timeout, common }));
        }
    } else if entry.following.len() == 0 {
        if let Some(equal) = entry.text.find("=") {
            let frnt = entry.text[..equal].trim().to_string();
            let (front, front_context) = get_context(&frnt, entry.location.line)?;
            let bck = &entry.text[equal+1..];
            let (back, back_context) = get_context(&bck, entry.location.line)?;
            return Ok(Box::new(FlashcardQuestion {
                front: split(&front, "/"),
                back: split(&back, "/"),
                front_context,
                back_context,
                timeout,
                common,
            }));
        } else {
            return Err(QuizError::Parse {
                line: entry.location.line,
                whole_entry: true,
                message: String::from("question has no answer"),
            });
        }
    } else {
        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            if _ordered != "true" && _ordered != "false" {
                let message = String::from(
                    "ordered field must be either 'true' or 'false'");
                return Err(QuizError::Parse {
                    line: entry.location.line,
                    whole_entry: true,
                    message,
                });
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

        let answer_list = entry.following.iter().map(|l| split(&l, "/")).collect();
        if ordered {
            return Ok(Box::new(OrderedListQuestion {
                text, answer_list, no_credit, common
            }));
        } else {
            return Ok(Box::new(ListQuestion {
                text, answer_list, no_credit, common,
            }));
        }
    };
}

#[derive(Debug)]
struct GlobalSettings {
    instructions: Option<String>,
    timeout: Option<u64>,
}


/// Read the initial settings from the file.
fn read_settings(reader: &mut QuizReader) -> Result<GlobalSettings> {
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
                    return Err(QuizError::Parse {
                        line: reader.line,
                        whole_entry: false,
                        message: String::from("no blank line after quiz settings"),
                    });
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
fn read_entry(path: &PathBuf, reader: &mut QuizReader) -> Result<Option<FileEntry>> {
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
                        return Err(QuizError::Parse {
                            line: reader.line,
                            whole_entry: false,
                            message: String::from("no blank line between questions"),
                        });
                    }
                }
            }
            Ok(Some(entry))
        },
        Some(_) => {
            Err(QuizError::Parse {
                line: reader.line,
                whole_entry: false,
                message: String::from("expected first line of question"),
            })
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
    pub fn read_line(self: &mut QuizReader) -> Result<Option<FileLine>> {
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
                Err(QuizError::Parse {
                    line: self.line,
                    whole_entry: false,
                    message: String::from("expected colon"),
                })
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


fn parse_u64(s: &str, line: usize) -> Result<u64> {
    u64::from_str(s)
        .map_err(|_| QuizError::Parse {
            line, whole_entry: true, message: String::from("could not parse integer"),
        })
}


fn get_context(line: &str, lineno: usize) -> Result<(String, Option<String>)> {
    if let Some(open) = line.find('[') {
        if let Some(_close) = line[open..].find(']') {
            let close = _close + open;
            let new_line = String::from(line[..open].trim());
            let context = String::from(line[open+1..close].trim());
            Ok((new_line, Some(context)))
        } else {
            Err(QuizError::Parse {
                line: lineno,
                whole_entry: false,
                message: String::from("expected ]"),
            })
        }
    } else {
        Ok((String::from(line), None))
    }
}
