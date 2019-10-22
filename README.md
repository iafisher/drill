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
described in the section below. You can also look at the `sample_quiz` file.

Now, run

```shell
$ popquiz -d path/to/quizzes take your-quiz-name
```

to take your quiz! The program will create a `results` directory alongside your quiz
to keep track of your results over time.

You can also try out these commands:
```shell
# Count the questions in a quiz.
$ popquiz count <name>

# See previous results for a quiz.
$ popquiz results <name>
```

If `<name>` is left out of any of these commands, it defaults to `main`.


## Quiz file format
Here's an example of a quiz file:

```
[1] Which English countess is regarded as the first computer programmer?
Ada Lovelace / Lady Lovelace / Ada, Countess of Lovelace

[2] Name the four Home Islands of Japan.
Hokkaido
Honshu
Shikoku
Kyushu
- tags: geography, japan

[3] Who were the first three Presidents of the United States, in order?
George Washington / Washington
John Adams / Adams
Thomas Jefferson / Jefferson
- ordered: true

[4] In what year did the Russo-Japanese War end?
1905
- choices: 1878 / 1945 / 1918 / 1908

[5] woman = la mujer
```

Each question begins with a line of the format `[id] text` and ends with a blank line
(or the end of the file). Each subsequent line of a question that does not begin with a
dash is a required answer to the question. Multiple variants of the same answer are
separated by slashes.

In the special case that the question has no subsequent non-dashed lines, the question
text is interpreted as a flashcard whose two sides are separated by an equal signs. The
right-hand side may have multiple variants.

Lines beginning with a dash (technically a dash and a space, so that you can have, e.g.,
`-5` be the answer to a question) are for metadata and extra configuration. They must
consist of a key not containing whitespace, followed by a colon, followed by at least
one non-whitespace character. The currently-recognized keys are

- `choices`: Turns the question into a multiple choice question.
- `nocredit`: Answers in this slash-separated list are counted as neither correct nor
              incorrect, e.g. if the question is "Name the five largest countries by
              area", you might not want to penalize naming the sixth largest country.
              Only recognized for questions with multiple answers.
- `ordered`: Answers must be supplied in the order given in the quiz file.
- `tags`: Comma-separated list of tags.

Keys not in this list are ignored.

The `id` in the first line of each question allows popquiz to keep track of your results
on each question even if you tweak the text of the question. It is conventionally a
number, but it can be any sequence of characters except for `]`. It must be unique
within a quiz file. Only change it when you change the question enough that previous
results become irrelevant.
