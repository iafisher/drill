/**
 * Functions and data structures for reading and writing quiz and results files in the
 * filesystem.
 *
 * Author:  Ian Fisher (iafisher@fastmail.com)
 * Version: October 2019
 */
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use super::common::{Location, QuizError, Result};
use super::quiz::{
    Answer, FlashcardQuestion, ListQuestion, MultipleChoiceQuestion, OrderedListQuestion, Question,
    QuestionCommon, QuestionResult, Quiz, QuizResult, ShortAnswerQuestion,
};

/// Load a `Quiz` object given its name.
pub fn load_quiz(fullname: &Path) -> Result<Quiz> {
    let old_results = load_results(fullname)?;
    parse(fullname, &old_results)
}

type StoredResults = HashMap<String, Vec<QuestionResult>>;
type ChoiceGroup = HashMap<String, Answer>;

pub fn load_results(fullname: &Path) -> Result<StoredResults> {
    let results_path = get_results_path(fullname)?;
    match fs::read_to_string(results_path) {
        Ok(data) => serde_json::from_str(&data).map_err(QuizError::Json),
        Err(_) => Ok(HashMap::new()),
    }
}

/// Save `results` to a file in a results directory, appending the results if previous
/// results have been recorded.
pub fn save_results(fullname: &Path, results: &QuizResult) -> Result<()> {
    let results_dir = get_results_dir_path(fullname)?;
    if !results_dir.as_path().exists() {
        fs::create_dir(&results_dir).map_err(QuizError::Io)?;
    }

    // Load old data, if it exists.
    let results_path = get_results_path(fullname)?;
    let data = fs::read_to_string(&results_path);
    let mut hash: BTreeMap<String, Vec<QuestionResult>> = match data {
        Ok(ref data) => serde_json::from_str(&data).map_err(QuizError::Json)?,
        Err(_) => BTreeMap::new(),
    };

    // Store the results as a map from the text of the questions to a list of individual
    // time-stamped results.
    for result in results.per_question.iter() {
        if !hash.contains_key(&result.id) {
            hash.insert(result.id.to_string(), Vec::new());
        }
        hash.get_mut(&result.id).unwrap().push(result.clone());
    }

    let serialized_results = serde_json::to_string_pretty(&hash).map_err(QuizError::Json)?;
    fs::write(&results_path, serialized_results)
        .or(Err(QuizError::CannotWriteToFile(results_path.clone())))?;
    Ok(())
}

fn get_results_dir_path(fullname: &Path) -> Result<PathBuf> {
    let mut builder = if let Some(parent) = fullname.parent() {
        parent.to_path_buf()
    } else {
        PathBuf::new()
    };
    builder.push("results");
    Ok(builder)
}

fn get_results_path(fullname: &Path) -> Result<PathBuf> {
    let shortname = fullname
        .file_name()
        .ok_or(QuizError::QuizNotFound(fullname.to_path_buf()))?;
    let shortname = shortname
        .to_str()
        .ok_or(QuizError::QuizNotFound(fullname.to_path_buf()))?;

    let mut builder = get_results_dir_path(fullname)?;
    builder.push(format!("{}_results.json", shortname));
    Ok(builder)
}

fn parse(path: &Path, old_results: &StoredResults) -> Result<Quiz> {
    let file = File::open(path).map_err(QuizError::Io)?;
    let mut reader = QuizReader::new(BufReader::new(file));
    let quiz_settings = read_settings(&mut reader)?;

    let mut questions = Vec::new();
    let mut used_ids = HashSet::new();
    let mut choice_groups = HashMap::new();
    loop {
        match read_entry(path, &mut reader) {
            Ok(Some(FileEntry::QuestionEntry(entry))) => {
                let q = entry_to_question(&entry, &choice_groups, old_results)?;
                if used_ids.contains(&q.get_common().id) {
                    return Err(QuizError::Parse {
                        line: entry.location.line,
                        whole_entry: false,
                        message: String::from("duplicate question ID"),
                    });
                }
                used_ids.insert(q.get_common().id.clone());
                questions.push(q);
            }
            Ok(Some(FileEntry::ChoiceGroupEntry(entry))) => {
                if choice_groups.contains_key(&entry.id) {
                    return Err(QuizError::Parse {
                        line: entry.location.line,
                        whole_entry: false,
                        message: String::from("duplicate choice group ID"),
                    });
                }
                choice_groups.insert(entry.id.clone(), entry.choices.clone());
            }
            Ok(None) => {
                break;
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(Quiz {
        instructions: quiz_settings.instructions,
        questions,
    })
}

fn entry_to_question(
    entry: &QuestionEntry,
    choice_groups: &HashMap<String, ChoiceGroup>,
    old_results: &StoredResults,
) -> Result<Box<Question>> {
    let lineno = entry.location.line;
    let tags = entry
        .attributes
        .get("tags")
        .map(|v| split(v, ","))
        .unwrap_or(Vec::new());

    let prior_results = old_results
        .get(&entry.id)
        .map(|v| v.clone())
        .unwrap_or(Vec::new());

    let common = QuestionCommon {
        id: entry.id.clone(),
        prior_results,
        tags,
        location: entry.location.clone(),
    };

    // TODO: Handle multiple question texts.
    let entry = entry.clone();
    let text = entry.text.clone();
    if entry.following.len() == 1 {
        check_fields(&entry.attributes, &["choices", "tags"], lineno)?;

        let answer = split(&entry.following[0], "/");
        if let Some(choices) = entry.attributes.get("choices") {
            return Ok(Box::new(MultipleChoiceQuestion {
                text,
                answer,
                choices: split(&choices, "/"),
                common,
            }));
        } else {
            return Ok(Box::new(ShortAnswerQuestion {
                text,
                answer,
                common,
            }));
        }
    } else if entry.following.len() == 0 {
        if let Some(equal) = entry.text.find("=") {
            check_fields(&entry.attributes, &["tags"], lineno)?;

            let frnt = entry.text[..equal].trim().to_string();
            let (front, front_context) = get_context(&frnt, lineno)?;
            let bck = &entry.text[equal + 1..];
            let (back, back_context) = get_context(&bck, lineno)?;
            return Ok(Box::new(FlashcardQuestion {
                front: split(&front, "/"),
                back: split(&back, "/"),
                front_context,
                back_context,
                common,
            }));
        } else if let Some(choice_group_name) = entry.attributes.get("choice-group") {
            if let Some(answer_code) = entry.attributes.get("choice-group-answer") {
                if let Some(choice_group) = choice_groups.get(choice_group_name) {
                    if let Some(answer) = choice_group.get(answer_code) {
                        // Copy all the possible choices, except for all the choices
                        // corresponding to the correct answer.
                        let mut choices = Vec::new();
                        for (choice_code, choice) in choice_group {
                            if choice_code == answer_code {
                                continue;
                            }

                            for choice_variant in choice.iter() {
                                choices.push(choice_variant.clone());
                            }
                        }
                        return Ok(Box::new(MultipleChoiceQuestion {
                            text,
                            answer: answer.clone(),
                            choices: choices,
                            common,
                        }));
                    } else {
                        return Err(QuizError::Parse {
                            line: lineno,
                            whole_entry: true,
                            message: String::from("choice group answer does not exist"),
                        });
                    }
                } else {
                    return Err(QuizError::Parse {
                        line: lineno,
                        whole_entry: true,
                        message: String::from("choice group does not exist"),
                    });
                }
            } else {
                return Err(QuizError::Parse {
                    line: lineno,
                    whole_entry: true,
                    message: String::from("question has choice-group but not choice-group-answer"),
                });
            }
        } else {
            return Err(QuizError::Parse {
                line: lineno,
                whole_entry: true,
                message: String::from("question has no answer"),
            });
        }
    } else {
        check_fields(&entry.attributes, &["nocredit", "ordered", "tags"], lineno)?;

        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            if _ordered != "true" && _ordered != "false" {
                let message = String::from("ordered field must be either 'true' or 'false'");
                return Err(QuizError::Parse {
                    line: lineno,
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
                text,
                answer_list,
                no_credit,
                common,
            }));
        } else {
            return Ok(Box::new(ListQuestion {
                text,
                answer_list,
                no_credit,
                common,
            }));
        }
    };
}

#[derive(Debug)]
struct GlobalSettings {
    instructions: Option<String>,
}

/// Read the initial settings from the file.
fn read_settings(reader: &mut QuizReader) -> Result<GlobalSettings> {
    let mut settings = GlobalSettings { instructions: None };
    let mut first_line = true;
    loop {
        match reader.read_line()? {
            Some(FileLine::Pair(key, val)) => {
                if key == "instructions" {
                    settings.instructions.replace(val);
                } else {
                    return Err(QuizError::Parse {
                        line: reader.line,
                        whole_entry: false,
                        message: format!("unexpected field '{}'", key),
                    });
                }
            }
            Some(FileLine::Blank) | None => {
                break;
            }
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
            }
        }
        first_line = false;
    }
    Ok(settings)
}

/// Read an entry from the file.
///
/// `Ok(Some(entry))` is returned on a successful read. `Ok(None)` is returned when the
/// end of file is reached. `Err(e)` is returned if a parse error occurs.
fn read_entry(path: &Path, reader: &mut QuizReader) -> Result<Option<FileEntry>> {
    // Loop over blank lines before the actual question.
    loop {
        match reader.read_line()? {
            Some(FileLine::First(id, text)) => {
                let mut entry = QuestionEntry {
                    id,
                    text,
                    following: Vec::new(),
                    attributes: HashMap::new(),
                    location: Location {
                        line: reader.line,
                        path: path.to_path_buf(),
                    },
                };
                loop {
                    let line = reader.read_line()?;
                    match line {
                        Some(FileLine::Blank) | None => {
                            break;
                        }
                        Some(FileLine::Following(line)) => {
                            entry.following.push(line);
                        }
                        Some(FileLine::Pair(key, value)) => {
                            entry.attributes.insert(key, value);
                        }
                        Some(FileLine::First(..)) => {
                            reader.push(line.unwrap());
                            break;
                        }
                        Some(_) => {
                            return Err(QuizError::Parse {
                                line: reader.line,
                                whole_entry: false,
                                message: String::from("unexpected line in question"),
                            });
                        }
                    }
                }
                return Ok(Some(FileEntry::QuestionEntry(entry)));
            }
            Some(FileLine::ChoiceGroup(id)) => {
                let mut entry = ChoiceGroupEntry {
                    id,
                    choices: ChoiceGroup::new(),
                    location: Location {
                        line: reader.line,
                        path: path.to_path_buf(),
                    },
                };
                loop {
                    let line = reader.read_line()?;
                    match line {
                        Some(FileLine::Blank) | None => {
                            break;
                        }
                        Some(FileLine::Pair(key, value)) => {
                            entry.choices.insert(key, split(&value, "/"));
                        }
                        Some(_) => {
                            return Err(QuizError::Parse {
                                line: reader.line,
                                whole_entry: false,
                                message: String::from("unexpected line in choice group"),
                            });
                        }
                    }
                }
                return Ok(Some(FileEntry::ChoiceGroupEntry(entry)));
            }
            Some(FileLine::Blank) => {
                continue;
            }
            Some(_) => {
                return Err(QuizError::Parse {
                    line: reader.line,
                    whole_entry: false,
                    message: String::from("expected first line of question"),
                });
            }
            None => {
                return Ok(None);
            }
        }
    }
}

fn split(s: &str, splitter: &str) -> Vec<String> {
    s.split(splitter).map(|w| w.trim().to_string()).collect()
}

enum FileLine {
    First(String, String),
    ChoiceGroup(String),
    Following(String),
    Pair(String, String),
    Blank,
}

enum FileEntry {
    QuestionEntry(QuestionEntry),
    ChoiceGroupEntry(ChoiceGroupEntry),
}

#[derive(Clone, Debug)]
struct QuestionEntry {
    id: String,
    text: String,
    following: Vec<String>,
    attributes: HashMap<String, String>,
    location: Location,
}

#[derive(Clone, Debug)]
struct ChoiceGroupEntry {
    id: String,
    choices: ChoiceGroup,
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
        Self {
            reader,
            pushed: None,
            line: 0,
        }
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
                let value = trimmed[colon + 1..].trim().to_string();
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
            let rest = &trimmed[brace + 1..];
            Ok(Some(FileLine::First(
                id.trim().to_string(),
                rest.trim().to_string(),
            )))
        } else if trimmed.starts_with("choice-group") {
            let rest = &trimmed["choice-group".len()..].trim();
            if rest.len() > 0 {
                Ok(Some(FileLine::ChoiceGroup(rest.to_string())))
            } else {
                Err(QuizError::Parse {
                    line: self.line,
                    whole_entry: false,
                    message: String::from("expected identifier"),
                })
            }
        } else {
            Ok(Some(FileLine::Following(trimmed.to_string())))
        }
    }
}

fn get_context(line: &str, lineno: usize) -> Result<(String, Option<String>)> {
    if let Some(open) = line.find('[') {
        if let Some(_close) = line[open..].find(']') {
            let close = _close + open;
            let new_line = String::from(line[..open].trim());
            let context = String::from(line[open + 1..close].trim());
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

fn check_fields(attrib: &HashMap<String, String>, allowed: &[&str], lineno: usize) -> Result<()> {
    for key in attrib.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(QuizError::Parse {
                line: lineno,
                whole_entry: true,
                message: format!("unexpected field '{}'", key),
            });
        }
    }
    Ok(())
}
