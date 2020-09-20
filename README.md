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

You can also try out these commands:
```shell
# Count the questions in a quiz.
$ drill --count <name>
# Count the number of questions per tag.
$ drill --count --list-tags

# See previous results for a quiz.
$ drill --results <name>
$ drill --results <name> -n 20 --sort best
$ drill --results <name> -n 20 --sort worst
$ drill --results <name> -n 20 --sort most
$ drill --results <name> -n 20 --sort least

# Search for a question.
$ drill --search <name> <keyword>

# See per-question history.
$ drill --history <name> <question-id>
```

If `<name>` isn't supplied and isn't followed by a positional argument, then it defaults to `main`.

drill is configurable with command-line flags. Run `drill --help` for details. For convenience, you can set an environment variable called `DRILL_HOME` to the directory containing your quizzes, and drill will read from this directory regardless of where it is invoked.


### In-quiz commands
If a question is erroneously marked incorrect (e.g., because you made a typo), you can mark it correct by entering `!!` at the next question prompt. On the next question, you can enter `!!` to mark the previous answer correct. You can also enter `!e` or `!edit` to open up the previous question in a text editor, e.g. in case there is a typo in the question text.

If all else fails, you can directly edit the results file that is created for each quiz.


## Development
Run the test suite with `./t`. Any arguments provided to `./t` will be passed on to `cargo test`.
