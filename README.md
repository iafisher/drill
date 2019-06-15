# iafisher/popquiz
A command-line program to take quizzes.

Create a quiz file in the format described in the section below, and then install and run the popquiz application with

```shell
$ git clone https://github.com/iafisher/popquiz.git
$ cd popquiz
$ cargo run -- path/to/quiz.json
```

You must have Rust and Cargo installed.


## Quiz file format
You create the quizzes yourself as JSON files. This section documents the format of the JSON, although note that since popquiz is under active development, the format may change in backwards-incompatible fashion without warning.

The root JSON object should have a single `questions` field mapping to an array of question objects:

```json
{
  "questions": [
    ...
  ]
}
```

The format of the question objects depends on the kind of questions. popquiz currently supports four question types:

- Short answer questions
- Unordered list questions
- Ordered list questions
- Multiple-choice questions


## Short answer questions
```json
{
  "kind": "ShortAnswer",
  "text": "Which English countess is regarded as the first computer programmer?",
  "answer": ["Ada Lovelace", "Lady Lovelace", "Ada, Countess of Lovelace"]
}
```

The `kind` field defaults to `"ShortAnswer"` so it is optional here. The `text` field is the text of the question, and the `answer` field is an array of acceptable answers. It can also be a single string.


## Unordered list questions
These are questions for which the quiz-taker must supply a list of answers, in any order.

```json
{
  "kind": "ListAnswer",
  "text": [
    "Name the four Home Islands of Japan.",
    "What are the four principal islands of the Japanese archipelago?"
  ],
  "tags": ["geography", "japan"],
  "answer_list": ["Hokkaido", "Honshu", "Shikoku", "Kyushu"]
}
```

Unordered list questions use an `answer_list` field instead of an `answer` field. For any question type, the `text` field may be an array of strings, to allow for multiple variations on the same question. All question types support a `tags` field which lets user filter questions with command-line options.


## Ordered list questions
These are questions for which the quiz-taker must supply a list of answers in a specified order.

```json
{
  "kind": "OrderedListAnswer",
  "text": "Who were the first three Presidents of the United States, in order?",
  "answer_list": [
    ["George Washington", "Washington"],
    ["John Adams", "Adams"],
    ["Thomas  Jefferson", "Jefferson"]
  ]
}
```

The format of ordered list questions is almost the same as for unordered list questions, except that the order of `answer_list` is significant. For both unordered and ordered list questions, the elements of `answer_list` may be arrays to allow for multiple acceptable variants of a single answer.


## Multiple-choice questions
```json
{
  "kind": "MultipleChoice",
  "text": "In what year did the Russo-Japanese War end?",
  "candidates": ["1878", "1945", "1918", "1908"],
  "answer": "1905"
}
```

The `candidates` field is for the incorrect answers to be displayed as options. It should **not** contain the correct answer, which goes in the `answer` field and must be a string.


For a complete example, see `sample.json` in the root of this repository.
