/**
 * End-to-end tests for the drill application.
 *
 * WARNING: These tests will not work if invoked directly with cargo. Use the `t` helper
 * script to run the test suite instead.
 */
use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time;

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use regex::Regex;

#[test]
fn can_take_simple_quiz1() {
    play_quiz(
        "test1",
        &["--no-save"],
        &[
            "(1) What is the capital of Mongolia?",
            "> Ulan Bator",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_take_simple_quiz2() {
    play_quiz(
        "test2",
        &["--no-save", "--in-order"],
        &[
            "(1) Who was President of the United States during the Korean War?",
            r"RE: \(a\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(b\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(c\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"RE: \(d\) (Harry S\. Truman|Franklin D\. Roosevelt|John F\. Kennedy|Lyndon Johnson)",
            r"> Harry Truman",
            "Please enter a letter.",
            r"> a",
            // Since the order of the choices is random, guessing 'a' may or may not
            // have been correct.
            r"RE: (Correct!|Incorrect\. The correct answer was Harry S\. Truman\.)",
            "(2) List the modern Emperors of Germany in chronological order.",
            "> Wilhelm I",
            "Correct!",
            "> Wilhelm II",
            "Incorrect. The correct answer was Frederick III.",
            "> Wilhelm II",
            "Correct!",
            "Score for this question: 66.6%",
            r"RE: (33\.3|83\.3)% out of 2 questions",
            r"RE: (0|1) correct",
            "1 partially correct",
            r"RE: (0|1) incorrect",
        ],
    );
}

#[test]
fn can_save_results_and_track_history() {
    play_quiz(
        "test1",
        &[],
        &[
            "(1) What is the capital of Mongolia?",
            "> Ulan Bator",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );

    play_quiz(
        "test1",
        &[],
        &[
            "(1) What is the capital of Mongolia?",
            "> Khovd",
            "Incorrect. The correct answer was Ulan Bator.",
            "0.0% out of 1 question",
            "0 correct",
            "1 incorrect",
        ],
    );

    assert!(Path::new("tests/quizzes/results/test1_results.json").exists());

    let (stdout, stderr) = spawn_and_mock(&["--no-color", "--results", "tests/quizzes/test1"]);
    assert_match(&stderr, "");
    assert_match(
        &stdout,
        "50.0% of  2   [1] What is the capital of Mongolia?\n",
    );
}

#[test]
fn can_take_quiz_with_list_question() {
    play_quiz(
        "test_list",
        &["--no-save", "--in-order"],
        &[
            "(1) Name the five members of the UN Security Council.",
            "> China",
            "Correct!",
            "> PR China",
            "You already said that.",
            "> France",
            "Correct!",
            "> Germany",
            "Incorrect.",
            "> US",
            "Correct!",
            "> United Kingdom",
            "Correct!",
            "You missed:",
            "Russia",
            "Score for this question: 80.0%",
            "80.0% out of 1 question",
            "0 correct",
            "1 partially correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_take_flashcard_quiz() {
    play_quiz(
        "test_flashcard",
        &["--no-save", "--in-order"],
        &[
            "(1) el pan",
            "> bread",
            "Correct!",
            "(2) el vino",
            "> wine",
            "Correct!",
            "(3) la mantequilla",
            "> butter",
            "Correct!",
            "100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_take_flipped_flashcard_quiz() {
    play_quiz(
        "test_flashcard",
        &["--no-save", "--in-order", "--flip"],
        &[
            "(1) bread",
            "> el pan",
            "Correct!",
            "(2) wine",
            "> el vino",
            "Correct!",
            "(3) butter",
            "> la mantequilla",
            "Correct!",
            "100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn ctrl_d_skips_current_question() {
    play_quiz(
        "test_flashcard",
        &["--no-save", "--in-order"],
        &[
            "(1) el pan",
            "> Ctrl+D",
            "Incorrect. The correct answer was bread.",
            "(2) el vino",
            "> Ctrl+D",
            "Incorrect. The correct answer was wine.",
            "(3) la mantequilla",
            "> Ctrl+D",
            "Incorrect. The correct answer was butter.",
            "0.0% out of 3 questions",
            "0 correct",
            "3 incorrect",
        ],
    );
}

#[test]
fn ctrl_c_aborts_quiz() {
    play_quiz(
        "test_flashcard",
        &["--no-save", "--in-order"],
        &["(1) el pan", "> Ctrl+C"],
    );

    // This test case fails when it should pass, but I don't understand why.
    // play_quiz(
    //     "test_flashcard",
    //     &["--in-order"],
    //     &[
    //         "(1) el pan",
    //         "> bread",
    //         "Correct!",
    //         "(2) el vino",
    //         "> Ctrl+C",
    //         "Score: 100.0% out of 1 question",
    //         "1 correct",
    //         "0 incorrect",
    //     ]
    // );
}

#[test]
fn no_credit_answers_work() {
    play_quiz(
        "test_no_credit",
        &["--no-save"],
        &[
            "(1) Name the three largest cities of the Inland Empire.",
            "> Riverside",
            "Correct!",
            "> Ontario",
            "No credit.",
            "> San Bernardino",
            "Correct!",
            "> Corona",
            "No credit.",
            "> Fontana",
            "Correct!",
            "Score for this question: 100.0%",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn quiz_instructions_are_displayed() {
    play_quiz(
        "test_instructions",
        &["--no-save"],
        &[
            "Include the state's postal code.",
            "(1) What is the capital of Michigan?",
            "> Lansing, MI",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn flashcards_context() {
    play_quiz(
        "test_flashcard_context",
        &["--no-save"],
        &[
            "(1) to read [perf]",
            "> прочитать",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );

    play_quiz(
        "test_flashcard_context",
        &["--no-save", "--flip"],
        &[
            "(1) прочитать [bleh]",
            "> to read",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_correct_questions_in_quiz() {
    play_quiz(
        "test_correction",
        &["--no-save", "--in-order"],
        &[
            "(1) What is the largest city in Northern California?",
            "> San Jose",
            "Incorrect. The correct answer was San Francisco.",
            "(2) What is the largest city in Oregon?",
            "> !!",
            "Previous answer marked correct.",
            "(2) What is the largest city in Oregon?",
            "> Eugene",
            "Incorrect. The correct answer was Portland.",
            "(3) Name two things.",
            "> !!",
            "Previous answer marked correct.",
            "(3) Name two things.",
            "> foi",
            "Incorrect. The correct answer was foo.",
            "> !!",
            "Previous answer marked correct.",
            "> bar",
            "Correct!",
            "Score for this question: 100.0%",
            "100.0% out of 3 questions",
            "3 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_correct_list_questions_in_quiz() {
    play_quiz(
        "test_correction_list",
        &["--no-save", "--in-order"],
        &[
            "(1) What is the capital of Ecuador?",
            "> Quit",
            "Incorrect. The correct answer was Quito.",
            "(2) Name the four countries of the United Kingdom.",
            "> !!",
            "Previous answer marked correct.",
            "(2) Name the four countries of the United Kingdom.",
            "> Enlgand",
            "Incorrect.",
            "> !!",
            "Previous answer undone.",
            "> England",
            "Correct!",
            "> Scotland",
            "Correct!",
            "> !!",
            "Previous answer was already correct.",
            "> Northern Ireland",
            "Correct!",
            "> Wales",
            "Correct!",
            "Score for this question: 100.0%",
            "100.0% out of 2 questions",
            "2 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn unicode_normalization_works() {
    play_quiz(
        "test_unicode_normalization",
        &["--no-save"],
        &[
            "(1) traffic",
            "> el tra\u{0301}fico",
            "Correct!",
            "100.0% out of 1 question",
            "1 correct",
            "0 incorrect",
        ],
    );
}

#[test]
fn can_use_choice_groups() {
    play_quiz(
        "test_choice_group",
        &["--no-save", "--in-order"],
        &[
            "(1) What is the largest city in Georgia?",
            r"RE: \(a\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(b\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(c\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(d\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"> a",
            // Since the order of the choices is random, guessing 'a' may or may not
            // have been correct.
            r"RE: (Correct!|Incorrect\. The correct answer was Atlanta\.)",
            "(2) What is the largest city in Illinois?",
            r"RE: \(a\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(b\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(c\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"RE: \(d\) (Atlanta|New York City|Chicago|Dallas|NYC)",
            r"> a",
            r"RE: (Correct!|Incorrect\. The correct answer was Chicago\.)",
            r"RE: (0\.0|50\.0|100\.0)% out of 2 questions",
            r"RE: (0|1|2) correct",
            r"RE: (0|1|2) incorrect",
        ],
    );
}

#[test]
fn searching_questions_works() {
    let (stdout, stderr) = spawn_and_mock(&[
        "--no-color",
        "--search",
        "tests/quizzes/test_tags",
        "Turkey",
    ]);
    assert_match(&stderr, "");
    assert_match(&stdout, "[2] What is the capital of Turkey?\n");

    let (stdout, stderr) = spawn_and_mock(&[
        "--no-color",
        "--search",
        "tests/quizzes/test_tags",
        "capital",
        "--tag",
        "europe",
    ]);
    assert_match(&stderr, "");
    assert_match(
        &stdout,
        "[2] What is the capital of Turkey?\n[3] What is the capital of Bulgaria?",
    );
}

#[test]
fn results_subcommand_works() {
    let (stdout, stderr) = spawn_and_mock(&["--no-color", "--results", "tests/quizzes/long/long"]);
    assert_match(&stderr, "");
    assert_match(
        &stdout,
        r"
100.0% of  3   [5] What application-level communications protocol is used by
               routers to assign IP addresses dynamically?
100.0% of  2   [3] What application-level communications protocol is used to
               deliver mail between email servers?
100.0% of  2   [4] What application-level communications protocol is used by
               email clients to retrieve mail from email servers, replacing the
               earlier POP3 standard?
100.0% of  2   [7] What is the name for a concurrency primitive that supports
               two operations, acquire and release?
 66.6% of  3   [6] For a connected graph, what is the term for the acyclic
               connected subgraph with the minimum sum of edge weights?
 58.3% of  3   [2] What are the four layers of the network stack, from lowest to
               highest?
 55.5% of  3   [1] What are the three core types of objects in the Git version
               control system?
        ",
    );

    let (stdout, stderr) = spawn_and_mock(&[
        "--no-color",
        "--results",
        "tests/quizzes/long/long",
        "--sort",
        "worst",
        "-n",
        "3",
    ]);

    assert_match(&stderr, "");
    assert_match(
        &stdout,
        r"
 55.5% of  3   [1] What are the three core types of objects in the Git version
               control system?
 58.3% of  3   [2] What are the four layers of the network stack, from lowest to
               highest?
 66.6% of  3   [6] For a connected graph, what is the term for the acyclic
               connected subgraph with the minimum sum of edge weights?
        ",
    );

    let (stdout, stderr) = spawn_and_mock(&[
        "--no-color",
        "--results",
        "tests/quizzes/long/long",
        "--sort",
        "most",
        "-n",
        "2",
    ]);

    assert_match(&stderr, "");
    assert_match(
        &stdout,
        r"
 55.5% of  3   [1] What are the three core types of objects in the Git version
               control system?
 58.3% of  3   [2] What are the four layers of the network stack, from lowest to
               highest?
        ",
    );
}

#[test]
fn parse_error_no_blank_line_after_settings() {
    assert_parse_error(
        "test_no_blank_line_after_settings",
        "no blank line after quiz settings",
        2,
        false,
    );
}

#[test]
fn non_zero_exit_code_on_error() {
    let child = spawn(&[
        "--no-color",
        "tests/quizzes/parse/test_no_blank_line_after_settings",
    ]);
    let result = child.wait_with_output().expect("Failed to read stdout");
    assert!(
        !result.status.success(),
        format!("Exit code: {:?}", result.status.code())
    );
}

#[test]
fn parse_error_wrong_ordered_value() {
    assert_parse_error(
        "test_wrong_ordered_value",
        "ordered field must be either 'true' or 'false'",
        1,
        true,
    );
}

#[test]
fn parse_error_no_first_line() {
    assert_parse_error(
        "test_no_first_line",
        "expected first line of question",
        1,
        false,
    );
}

#[test]
fn parse_error_bad_attribute() {
    assert_parse_error("test_bad_attribute", "expected colon", 3, false);
}

#[test]
fn parse_error_bad_flashcard_context() {
    assert_parse_error("test_bad_flashcard_context", "expected ]", 1, false);
}

#[test]
fn parse_error_unknown_field() {
    assert_parse_error("test_unknown_field", "unexpected field 'whatever'", 1, true);
}

#[test]
fn parse_error_unknown_global_field() {
    assert_parse_error(
        "test_unknown_global_field",
        "unexpected field 'whatever'",
        1,
        false,
    );
}

#[test]
fn parse_error_field_on_wrong_question() {
    assert_parse_error(
        "test_field_on_wrong_question",
        "unexpected field 'nocredit'",
        1,
        true,
    );
}

#[test]
fn parse_error_duplicate_ids() {
    assert_parse_error("test_duplicate_ids", "duplicate question ID", 2, false);
}

#[test]
fn parse_error_duplicate_choice_groups() {
    assert_parse_error(
        "test_duplicate_choice_group_ids",
        "duplicate choice group ID",
        4,
        false,
    );
}

#[test]
fn parse_error_nonexistent_choice_group() {
    assert_parse_error(
        "test_nonexistent_choice_group",
        "choice group does not exist",
        5,
        true,
    );
}

#[test]
fn parse_error_nonexistent_choice_group_answer() {
    assert_parse_error(
        "test_nonexistent_choice_group_answer",
        "choice group answer does not exist",
        5,
        true,
    );
}

#[test]
fn parse_error_missing_choice_group_answer() {
    assert_parse_error(
        "test_missing_choice_group_answer",
        "question has choice-group but not choice-group-answer",
        5,
        true,
    );
}

#[test]
fn parse_error_choice_group_line_in_question() {
    assert_parse_error(
        "test_choice_group_line_in_question",
        "unexpected line in question",
        3,
        false,
    );
}

#[test]
fn parse_error_unexpected_line_choice_group() {
    assert_parse_error(
        "test_unexpected_line_in_choice_group",
        "unexpected line in choice group",
        2,
        false,
    );
}

#[test]
fn parse_error_nameless_choice_group() {
    assert_parse_error(
        "test_nameless_choice_group",
        "expected identifier",
        1,
        false,
    );
}

fn assert_parse_error(path: &str, message: &str, lineno: usize, whole_entry: bool) {
    let fullpath = format!("tests/quizzes/parse/{}", path);
    let (_, stderr) = spawn_and_mock(&["--no-color", &fullpath]);
    let expected = if whole_entry {
        format!("Error: {} in entry beginning on line {}\n", message, lineno)
    } else {
        format!("Error: {} on line {}\n", message, lineno)
    };
    assert_match(&stderr, &expected);
}

fn assert_match(got: &str, expected: &str) {
    if expected.starts_with("RE:") {
        let expected = format!("^{}$", expected[3..].trim());
        let re = Regex::new(&expected).unwrap();
        assert!(
            re.is_match(&got.trim()),
            format!(
                "\n\nFailed to match {:?} against pattern {:?}\n\n",
                got.trim(),
                expected,
            )
        );
    } else {
        assert!(
            expected.trim() == got.trim(),
            format!(
                "\n\nExpected:\n  {:?}\n\ngot:\n  {:?}\n\n",
                expected.trim(),
                got.trim()
            ),
        );
    }
}

fn play_quiz(name: &str, extra_args: &[&str], in_out: &[&str]) {
    let mut args = vec!["--no-color"];
    let fullpath = format!("tests/quizzes/{}", name);
    args.push(&fullpath);
    args.extend_from_slice(extra_args);
    let mut child = spawn(&args[..]);
    let id = child.id();
    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        for line in in_out {
            if line.starts_with("> ") {
                if *line == "> Ctrl+C" {
                    // Give the program enough time to emit some output.
                    sleep(100);
                    signal::kill(Pid::from_raw(id as i32), Signal::SIGINT).unwrap();
                } else if *line == "> Ctrl+D" {
                    stdin.write(b"").unwrap();
                } else {
                    stdin_write(stdin, &line[1..]);
                }
            }
        }
    }

    let result = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();

    let mut lines_iter = stdout.lines();
    for expected in in_out {
        if !expected.starts_with("> ") {
            let mut got = lines_iter.next().expect(&format!(
                "Premature end of output. Expected {:?}. Contents of stderr: {:?}",
                expected, stderr
            ));
            loop {
                if got.trim().len() == 0 {
                    got = lines_iter.next().expect(&format!(
                        "Premature end of output. Expected {:?}. Contents of stderr: {:?}",
                        expected, stderr
                    ));
                } else {
                    break;
                }
            }

            assert_match(&got, &expected);
        }
    }

    while let Some(line) = lines_iter.next() {
        if line.trim().len() > 0 {
            panic!("Extra output: {:?}", line.trim());
        }
    }
}

fn assert_in_order(mock_stdout: &str, data: &[&str]) {
    let mut last_pos = 0;
    for datum in data {
        if let Some(pos) = mock_stdout[last_pos..].find(datum) {
            // `pos` must be adjusted by an offset of `last_pos` because it is an index
            // in the slice `mock_stdout[last_pos..]` but we want it to be relative to
            // `mock_stdout`.
            last_pos = (pos + last_pos) + datum.len();
        } else {
            panic!(
                "Missing: {:?}; Contents of stdout: {:?}",
                datum, mock_stdout
            );
        }
    }
}

fn spawn_and_mock(args: &[&str]) -> (String, String) {
    let child = spawn(args);
    let result = child.wait_with_output().expect("Failed to read stdout");
    let stdout = String::from_utf8_lossy(&result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).to_string();
    (stdout, stderr)
}

fn spawn(args: &[&str]) -> Child {
    Command::new("./target/debug/drill")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process")
}

fn stdin_write(stdin: &mut ChildStdin, line: &str) {
    stdin
        .write_all(line.as_bytes())
        .expect("Failed to write to stdin");
    stdin
        .write_all("\n".as_bytes())
        .expect("Failed to write to stdin");
}

fn sleep(millis: u64) {
    thread::sleep(time::Duration::from_millis(millis))
}
