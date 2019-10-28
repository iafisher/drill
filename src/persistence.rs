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
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use super::common::{Location, QuizError, Result};
use super::quiz::{
    FlashcardQuestion, ListQuestion, MultipleChoiceQuestion, OrderedListQuestion,
    Question, QuestionCommon, QuestionResult, Quiz, QuizResult, ShortAnswerQuestion};


/// Load a `Quiz` object given its name.
pub fn load_quiz(fullname: &Path) -> Result<Quiz> {
    let old_results = load_results(fullname)?;
    parse(fullname, &old_results)
}


type StoredResults = HashMap<String, Vec<QuestionResult>>;


pub fn load_results(fullname: &Path) -> Result<StoredResults> {
    let results_path = get_results_path(fullname)?;
    match fs::read_to_string(results_path) {
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
pub fn save_results(fullname: &Path, results: &QuizResult) -> Result<()> {
    let results_dir = get_results_dir_path(fullname)?;
    if !results_dir.as_path().exists() {
        fs::create_dir(&results_dir).map_err(QuizError::Io)?;
    }

    // Load old data, if it exists.
    let results_path = get_results_path(fullname)?;
    let data = fs::read_to_string(&results_path);
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
    let shortname = fullname.file_name()
        .ok_or(QuizError::QuizNotFound(fullname.to_path_buf()))?;
    let shortname = shortname.to_str()
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
    loop {
        match read_entry(path, &mut reader) {
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

    let lineno = entry.location.line;
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
        location: entry.location.clone(),
    };

    let timeout = if let Some(_timeout) = entry.attributes.get("timeout") {
        Some(parse_u64(_timeout, lineno)?)
    } else {
        settings.timeout
    };

    let script = settings.script.as_ref().or(entry.attributes.get("script"));
    let entry = if let Some(script) = script {
        entry_from_script(entry, script)?
    } else {
        entry.clone()
    };

    // TODO: Handle multiple question texts.
    let text = entry.text.clone();
    if entry.following.len() == 1 {
        check_fields(&entry.attributes, &["choices", "tags", "timeout"], lineno)?;

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
            check_fields(&entry.attributes, &["tags", "timeout"], lineno)?;

            let frnt = entry.text[..equal].trim().to_string();
            let (front, front_context) = get_context(&frnt, lineno)?;
            let bck = &entry.text[equal+1..];
            let (back, back_context) = get_context(&bck, lineno)?;
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
                line: lineno,
                whole_entry: true,
                message: String::from("question has no answer"),
            });
        }
    } else {
        check_fields(&entry.attributes, &["nocredit", "ordered", "tags"], lineno)?;

        let ordered = if let Some(_ordered) = entry.attributes.get("ordered") {
            if _ordered != "true" && _ordered != "false" {
                let message = String::from(
                    "ordered field must be either 'true' or 'false'");
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
                text, answer_list, no_credit, common
            }));
        } else {
            return Ok(Box::new(ListQuestion {
                text, answer_list, no_credit, common,
            }));
        }
    };
}


/// Create a new entry from the results of running a script.
fn entry_from_script(entry: &FileEntry, script_name: &str) -> Result<FileEntry> {
    let mut script_path = if let Some(parent) = entry.location.path.parent() {
        parent.to_path_buf()
    } else {
        PathBuf::new()
    };
    script_path.push(script_name);

    let line1 = entry.text.clone();
    let line2 = entry.following.join("\n");
    let stdout = run_script(&script_path, &line1, &line2)
        .map_err(|e| QuizError::Parse {
            line: entry.location.line,
            whole_entry: true,
            message: format!("could not run script {} ({})", script_name, e),
        })?;

    let mut lines: Vec<String> = stdout.lines().map(|s| String::from(s)).collect();
    if lines.len() >= 2 {
        let mut attributes = entry.attributes.clone();
        attributes.remove("script");
        let text = lines.remove(0);
        Ok(FileEntry {
            id: entry.id.clone(),
            text,
            following: lines,
            attributes,
            location: entry.location.clone(),
        })
    } else {
        Err(QuizError::Parse {
            line: entry.location.line,
            whole_entry: true,
            message: format!("script {} did not print two or more lines", script_name),
        })
    }
}


fn run_script(script_path: &Path, arg1: &str, arg2: &str) -> io::Result<String> {
    let result = Command::new(script_path)
        .arg(arg1)
        .arg(arg2)
        .stdout(Stdio::piped())
        .output()?;

    Ok(String::from_utf8_lossy(&result.stdout).to_string())
}


#[derive(Debug)]
struct GlobalSettings {
    instructions: Option<String>,
    script: Option<String>,
    timeout: Option<u64>,
}


/// Read the initial settings from the file.
fn read_settings(reader: &mut QuizReader) -> Result<GlobalSettings> {
    let mut settings = GlobalSettings { 
        instructions: None, script: None, timeout: None,
    };
    let mut first_line = true;
    loop {
        match reader.read_line()? {
            Some(FileLine::Pair(key, val)) => {
                if key == "instructions" {
                    settings.instructions.replace(val);
                } else if key == "timeout" {
                    settings.timeout.replace(parse_u64(&val, reader.line)?);
                } else if key == "script" {
                    settings.script.replace(val);
                } else {
                    return Err(QuizError::Parse {
                        line: reader.line,
                        whole_entry: false,
                        message: format!("unexpected field '{}'", key),
                    });
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
fn read_entry(path: &Path, reader: &mut QuizReader) -> Result<Option<FileEntry>> {
    match reader.read_line()? {
        Some(FileLine::First(id, text)) => {
            let mut entry = FileEntry {
                id,
                text,
                following: Vec::new(),
                attributes: HashMap::new(),
                location: Location { line: reader.line, path: path.to_path_buf() },
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

#[derive(Clone, Debug)]
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


fn parse_u64(s: &str, lineno: usize) -> Result<u64> {
    u64::from_str(s)
        .map_err(|_| QuizError::Parse {
            line: lineno,
            whole_entry: true,
            message: String::from("could not parse integer"),
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


fn check_fields(
    attrib: &HashMap<String, String>, allowed: &[&str], lineno: usize) -> Result<()> {

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
