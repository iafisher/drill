# iafisher/drill
Quiz yourself on trivia, general knowledge, foreign-language vocabulary etc. from the
command-line used spaced repetition.

**DISCLAIMER**: While anyone is welcome to use this tool, it is primarily for my
personal use and I do not guarantee that backwards compability will be maintained.


## Installation and usage
Installation requires [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/stable/cargo/).

```shell
$ cargo install --git https://github.com/iafisher/drill.git
```

Make a directory for your quizzes and create a quiz for yourself following the format
in `sample_quiz`.

Now, run

```shell
$ drill path/to/quiz
```

to take your quiz! The program will create a `results` directory alongside your quiz
to keep track of your results over time.

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

drill is configurable with command-line flags. Run `drill --help` for details.


### In-quiz commands
While taking a quiz, you may find that one of your answers is erroneously marked
incorrect. On the next question, you can enter `!!` to mark the previous answer correct.
You can also enter `!e` or `!edit` to open up the previous question in a text editor,
e.g. in case there is a typo in the question text.

If all else fails, you can directly edit the results file that is created for each quiz.


## Development
Run the test suite with `./t`. Any arguments provided to `./t` will be passed on to `cargo test`.
