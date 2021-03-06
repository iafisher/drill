# iafisher/drill
A command-line tool to learn information through spaced repetition.


## Installation and usage
Installation requires [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/stable/cargo/).

```shell
$ cargo install --git https://github.com/iafisher/drill.git
```

Make a directory for your quizzes and create a quiz for yourself following the format in `sample.quiz`.

Now, run

```shell
$ drill path/to/quiz
```

to take your quiz! The program will create a `results` directory alongside your quiz to keep track of your results over time.

drill is configurable with command-line flags. Run `drill --help` for details. For convenience, you can set an environment variable called `DRILL_HOME` to the directory containing your quizzes, and drill will read from this directory regardless of where it is invoked.


### In-quiz commands
If a question is erroneously marked incorrect (e.g., because you made a typo), you can mark it correct by entering `!!` at the next question prompt. On the next question, you can enter `!!` to mark the previous answer correct. You can also enter `!e` or `!edit` to open up the previous question in a text editor, e.g. in case there is a typo in the question text.


## Development
Run the test suite with `./t`. Any arguments provided to `./t` will be passed on to `cargo test`.
