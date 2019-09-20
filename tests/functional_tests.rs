use std::io;

use popquiz;


#[test]
fn can_take_test1_quiz() {
    let mut options = popquiz::QuizTakeOptions::new();
    options.name = s(".test1");

    let responses = vec![
        s("Ulan Bator\n"),
        s("no\n"),
    ];

    let mut mock_stdin = MockStdin { responses };
    let mut mock_stdout = MockStdout { sink: String::new() };

    let result = popquiz::main_take(&mut mock_stdout, &mut mock_stdin, options);

    assert!(result.is_ok());
    assert!(mock_stdin.responses.len() == 0);

    assert_in_order(
        &mock_stdout,
        &[
            "What is the capital of Mongolia?",
            "100.0%",
            "1 correct",
            "0 partially correct",
            "0 incorrect",
            "0 ungraded",
        ]
    );
}

#[test]
fn can_take_test2_quiz() {
    let mut options = popquiz::QuizTakeOptions::new();
    options.name = s(".test2");
    options.in_order = true;

    let responses = vec![
        s("a\n"),
        s("Wilhelm I\n"),
        s("Wilhelm II\n"),
        s("Wilhelm II\n"),
        s("no\n"),
    ];

    let mut mock_stdin = MockStdin { responses };
    let mut mock_stdout = MockStdout { sink: String::new() };

    let result = popquiz::main_take(&mut mock_stdout, &mut mock_stdin, options);

    assert!(result.is_ok());
    assert!(mock_stdin.responses.len() == 0);

    assert_in_order(
        &mock_stdout,
        &[
            "Who was President of the United States during the Korean War?",
            "List the modern Emperors of Germany in chronological order.",
            "Incorrect. The correct answer was Frederick III.",
            "Score for this question: 66.7%",
            "1 partially correct",
            "0 ungraded",
        ]
    );

    // Since the order of multiple-choice answers is random, we don't know whether
    // guessing 'a' was right or not.
    assert!(
        mock_stdout.has("1 incorrect") ||
        mock_stdout.has("1 correct")
    );
}

#[test]
fn can_take_flashcard_quiz() {
    let mut options = popquiz::QuizTakeOptions::new();
    options.name = s(".test_flashcard");
    options.in_order = true;

    let responses = vec![s("bread"), s("wine"), s("butter"), s("no\n")];

    let mut mock_stdin = MockStdin { responses };
    let mut mock_stdout = MockStdout { sink: String::new() };

    let result = popquiz::main_take(&mut mock_stdout, &mut mock_stdin, options);

    assert!(result.is_ok());
    assert!(mock_stdin.responses.len() == 0);

    assert_in_order(
        &mock_stdout,
        &[
            "el pan",
            "el vino",
            "la mantequilla",
            "100.0%",
        ]
    );
}

#[test]
fn can_take_flipped_flashcard_quiz() {
    let mut options = popquiz::QuizTakeOptions::new();
    options.name = s(".test_flashcard");
    options.in_order = true;
    options.flip = true;

    let responses = vec![s("el pan"), s("el vino"), s("la mantequilla"), s("no\n")];

    let mut mock_stdin = MockStdin { responses };
    let mut mock_stdout = MockStdout { sink: String::new() };

    let result = popquiz::main_take(&mut mock_stdout, &mut mock_stdin, options);

    assert!(result.is_ok());
    assert!(mock_stdin.responses.len() == 0);

    assert_in_order(
        &mock_stdout,
        &[
            "bread",
            "wine",
            "butter",
            "100.0%",
        ]
    );
}

fn s(mystr: &str) -> String {
    String::from(mystr)
}

fn assert_in_order(mock_stdout: &MockStdout, data: &[&str]) {
    assert!(
        mock_stdout.has_in_order(data), "Contents of stdout: {:?}", mock_stdout.sink
    );
}

struct MockStdin {
    responses: Vec<String>,
}

struct MockStdout {
    sink: String,
}

impl MockStdout {
    fn has(&self, datum: &str) -> bool {
        self.sink.contains(datum)
    }

    fn has_in_order(&self, data: &[&str]) -> bool {
        let mut last_pos = 0;
        for datum in data {
            if let Some(pos) = self.find(datum) {
                if pos < last_pos {
                    return false;
                } else {
                    last_pos = pos;
                }
            } else {
                return false;
            }
        }
        true
    }

    fn find(&self, datum: &str) -> Option<usize> {
        self.sink.find(datum)
    }
}

impl popquiz::MyReadline for MockStdin {
    fn read_line(&mut self, _prompt: &str) -> Result<String, popquiz::QuizError> {
        Ok(self.responses.remove(0))
    }
}

impl io::Write for MockStdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let as_utf8 = String::from_utf8(buf.to_vec()).unwrap();
        self.sink.push_str(&as_utf8);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
