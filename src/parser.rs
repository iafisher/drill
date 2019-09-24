/**
 * Implementation of the v2 parser for quizzes in the new textual format.
 *
 * Author:  Ian Fisher (iafisher@protonmail.com)
 * Version: September 2019
 */
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

use super::quiz;


struct QuestionAttribute {
    field: String,
    value: String,
    line: usize,
    // Is it preceded by a dash?
    dashed: bool,
}

type QuestionEntry = Vec<QuestionAttribute>;


pub fn parse(reader: &mut BufReader<File>) -> quiz::Quiz {
    let entries = read_file(reader);
    let mut questions = Vec::new();
    for entry in entries.iter() {
        if entry.len() < 2 {
            continue;
        }

        if entry[0].field == "q" && entry[1].field == "a" {
            questions.push(quiz::Question::new(&entry[0].value, &entry[1].value));
        }
    }
    quiz::Quiz { default_kind: None, instructions: None, questions }
}


fn read_file(reader: &mut BufReader<File>) -> Vec<QuestionEntry> {
    let mut entries = Vec::new();

    loop {
        if let Some(entry) = read_entry(reader) {
            entries.push(entry);
        } else {
            break;
        }
    }

    entries
}


fn read_entry(reader: &mut BufReader<File>) -> Option<QuestionEntry> {
    let mut entry = QuestionEntry::new();
    loop {
        if let Some(line) = read_line(reader) {
            if line.len() == 0 {
                break;
            }

            if let Some(colon_pos) = line.find(":") {
                let (field, value) = line.split_at(colon_pos);

                let trimmed_value = value[1..].trim().to_string();
                if field.starts_with("- ") {
                    let trimmed_field = field[2..].trim().to_string();
                    entry.push(QuestionAttribute {
                        field: trimmed_field,
                        value: trimmed_value,
                        line: 0,
                        dashed: true,
                    });
                } else {
                    let trimmed_field = field.trim().to_string();
                    entry.push(QuestionAttribute {
                        field: trimmed_field,
                        value: trimmed_value,
                        line: 0,
                        dashed: false,
                    });
                }
            } else {
                // TODO: Return an error.
            }
        } else {
            if entry.len() > 0 {
                break;
            } else {
                return None;
            }
        }
    }
    Some(entry)
}


fn read_line(reader: &mut BufReader<File>) -> Option<String> {
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap() == 0 {
        return None;
    }

    line = line.trim().to_string();
    if line.starts_with("#") {
        // Move to the next line
        read_line(reader)
    } else {
        Some(line)
    }
}
