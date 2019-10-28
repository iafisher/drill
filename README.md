# iafisher/popquiz
A command-line program to take quizzes.

**DISCLAIMER**: While anyone is welcome to use this tool, it is primarily for my
personal use and I do not guarantee that backwards compability will be maintained.


## Installation
Installation requires [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/stable/cargo/).

```shell
$ cargo install --git https://github.com/iafisher/popquiz.git
```

Make a directory for your quizzes and create a quiz for yourself following the format
in `sample_quiz`.

Now, run

```shell
$ popquiz take path/to/quiz
```

to take your quiz! The program will create a `results` directory alongside your quiz
to keep track of your results over time.

You can also try out these commands:
```shell
# Count the questions in a quiz.
$ popquiz count <name>
# Count the number of questions per tag.
$ popquiz count --list-tags

# See previous results for a quiz.
$ popquiz results <name>
$ popquiz results <name> -n 20 --sort best
$ popquiz results <name> -n 20 --sort worst
$ popquiz results <name> -n 20 --sort most
$ popquiz results <name> -n 20 --sort least

# Search for a question.
$ popquiz search <name> <keyword>

# See per-question history.
$ popquiz history <name> <question-id>
```

If `<name>` is left out of any of these commands, it defaults to `main`.


## In-quiz commands
While taking a quiz, you may find that one of your answers is erroneously marked
incorrect. On the next question, you can enter `!!` to mark the previous answer correct.
You can also enter `!e` or `!edit` to open up the previous question in a text editor,
e.g. in case there is a typo in the question text.

If all else fails, you can directly edit the results file that is created for each quiz.
