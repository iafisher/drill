# iafisher/popquiz
A command-line program to take quizzes.

Install and run the popquiz application with (you must have Rust and Cargo installed):

```shell
$ git clone https://github.com/iafisher/popquiz.git
$ cd popquiz
$ cargo run -- edit <name>
```

The last command will open up an editor for you to create a new quiz. Follow the format described in the section below. When done, check out these commands:

```shell
# Take a quiz.
$ cargo run -- take <name>

# Count the questions in a quiz.
$ cargo run -- count <name>

# See previous results for a quiz.
$ cargo run -- results <name>

# Edit a quiz.
$ cargo run -- edit <name>

# Delete a quiz.
$ cargo run -- delete <name>
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
- `ordered`: Answers must be supplied in the order given in the quiz file.
- `tags`: Comma-separated list of tags.

Keys not in this list are ignored.

The `id` in the first line of each question allows popquiz to keep track of your results
on each question even if you tweak the text of the question. It is conventionally a
number, but it can be any sequence of characters except for `]`. It must be unique
within a quiz file. Only change it when you change the question enough that previous
results become irrelevant.

For the old, JSON format of version 1, see [here](https://github.com/iafisher/popquiz/blob/52143169f9ffdfd1d3d029c3a3200f2c488476ea/README.md).

If you need to automatically migrate your quizzes from the version 1 format to the
version 2 format, use the interactive `./tools/migrate` script. It will migrate both
your quizzes and your quiz result files.
