import argparse
import contextlib
import os
import sqlite3
import sys


def main(old, new, *, overwrite=False):
    quiz = parse_quiz(old)

    if overwrite:
        with contextlib.suppress(FileNotFoundError):
            os.remove(new)

    exists = os.path.exists(new)
    db = sqlite3.connect(new)
    if not exists:
        create_database_tables(db)

    copy_quiz_to_database(db, quiz)

    db.commit()
    db.close()


def create_database_tables(db):
    db.execute(
        """
        CREATE TABLE quizzes(
          id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
          name TEXT UNIQUE NOT NULL CHECK(name != ''),
          instructions TEXT NOT NULL,
          version TEXT NOT NULL CHECK(version != ''),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )

    db.execute(
        """
        CREATE TABLE questions(
          id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
          quiz INTEGER NOT NULL REFERENCES quizzes,
          text TEXT NOT NULL CHECK (text != ''),
          type TEXT NOT NULL CHECK(
            type = 'short answer' OR
            type = 'ordered' OR
            type = 'unordered' OR
            type = 'multiple choice' OR
            type = 'flashcard'
          ),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )

    db.execute(
        """
        CREATE TABLE answers(
          question INTEGER NOT NULL REFERENCES questions,
          text TEXT NOT NULL CHECK(text != ''),
          correct BOOLEAN NOT NULL DEFAULT 1,
          no_credit BOOLEAN NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )

    db.execute(
        """
        CREATE TABLE tags(
          question INTEGER NOT NULL REFERENCES questions,
          name TEXT NOT NULL CHECK(name != ''),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        """
    )


def copy_quiz_to_database(db, quiz):
    cursor = db.cursor()
    cursor.execute(
        """
        INSERT INTO
          quizzes(name, instructions, version)
        VALUES
          (:name, :instructions, '1.0')
        """,
        {"name": quiz.name, "instructions": quiz.instructions or ""},
    )
    quiz_id = cursor.lastrowid

    for question in quiz.questions:
        cursor.execute(
            """
            INSERT INTO
              questions(quiz, text, type)
            VALUES
              (:quiz, :text, :type)
            """,
            {"quiz": quiz_id, "text": question.text, "type": question.type},
        )
        question_id = cursor.lastrowid

        for answer in question.answers:
            cursor.execute(
                """
                INSERT INTO
                  answers(question, text, no_credit, correct)
                VALUES
                  (:question, :text, :no_credit, :correct)
                """,
                {
                    "question": question_id,
                    "text": answer.text,
                    "no_credit": answer.no_credit,
                    "correct": answer.correct,
                },
            )

        for tag in question.tags:
            cursor.execute(
                """
                INSERT INTO
                  tags(question, name)
                VALUES
                  (:question, :name)
                """,
                {"question": question_id, "name": tag},
            )


def parse_quiz(path):
    with open(path, "r", encoding="utf8") as f:
        lines = f.readlines()

    i = skip_whitespace(lines, 0)

    instructions_prefix = "- instructions:"
    if lines[i].startswith(instructions_prefix):
        instructions = lines[i][len(instructions_prefix) :].strip()
        i = skip_whitespace(lines, i + 1)
    else:
        instructions = None

    questions = []
    while i < len(lines):
        question, i = parse_question(lines, i)
        if question is not None:
            questions.append(question)
        else:
            print(
                f"Warning: could not parse question ending at line {i + 1}.",
                file=sys.stderr,
            )
        i = skip_whitespace(lines, i)

    return Quiz(
        name=os.path.basename(path), questions=questions, instructions=instructions
    )


def parse_question(lines, i):
    left_bracket_index = lines[i].find("]")
    if left_bracket_index == -1:
        return None, skip_non_whitespace(lines, i + 1)

    text = lines[i][left_bracket_index + 1 :].strip()
    i += 1
    answers_as_strings = []
    while (
        i < len(lines)
        and lines[i]
        and not lines[i].isspace()
        and not lines[i].startswith("-")
        and not lines[i].startswith("[")
    ):
        answers_as_strings.append(lines[i].strip())
        i += 1

    no_credit = set()
    choices = []
    ordered = False
    tags = []
    while (
        i < len(lines)
        and lines[i]
        and not lines[i].isspace()
        and lines[i].startswith("-")
    ):
        key, value = lines[i][1:].split(":", maxsplit=1)
        key = key.strip()
        value = value.strip()

        parse_error = False
        if key == "nocredit":
            for s in value.split("/"):
                no_credit.add(s.strip())
        elif key == "ordered":
            if value == "true":
                ordered = True
            elif value == "false":
                ordered = False
            else:
                parse_error = True
        elif key == "choices":
            choices = [choice.strip() for choice in value.split("/")]
        elif key == "tags":
            tags = [tag.strip() for tag in value.split(",")]
        else:
            parse_error = True

        if parse_error:
            return None, skip_non_whitespace(lines, i + 1)

        i += 1

    if answers_as_strings:
        answers = [
            Answer(text, no_credit=bool(text in no_credit), correct=True)
            for text in answers_as_strings
        ]

        if len(answers) > 1:
            type = "ordered" if ordered else "unordered"
        else:
            if choices:
                type = "multiple choice"

                for choice in choices:
                    answers.append(Answer(choice, no_credit=False, correct=False))
            else:
                type = "short answer"

        question = Question(text, type=type, answers=answers, tags=tags)
    else:
        if "=" in text:
            question = Question(text, type="flashcard", answers=[])
        else:
            question = None

    return question, i


def skip_whitespace(lines, i):
    while i < len(lines) and (not lines[i] or lines[i].isspace()):
        i += 1

    return i


def skip_non_whitespace(lines, i):
    while i < len(lines) and lines[i] and not lines[i].isspace():
        i += 1

    return i


class Quiz:
    def __init__(self, name, questions, instructions):
        self.name = name
        self.questions = questions
        self.instructions = instructions

    def __repr__(self):
        return (
            f"Quiz(name={self.name!r}, questions={self.questions!r}, "
            + f"instructions={self.instructions!r})"
        )


class Question:
    def __init__(self, text, type, answers, tags):
        self.text = text
        self.type = type
        self.answers = answers
        self.tags = tags

    def __repr__(self):
        return (
            f"Question(text={self.text!r}, type={self.type!r}, "
            + f"answers={self.answers!r}, tags={self.tags!r})"
        )


class Answer:
    def __init__(self, text, no_credit, correct):
        self.text = text
        self.no_credit = no_credit
        self.correct = correct

    def __repr__(self):
        return (
            f"Answer(text={self.text!r}, no_credit={self.no_credit!r}, "
            + f"correct={self.correct!r})"
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Migrate a quiz file from the old text format to the new SQLite "
        + "format."
    )
    parser.add_argument("--old", help="Path to the quiz file.", required=True)
    parser.add_argument(
        "--new", help="Path at which to create the SQLite database.", required=True
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        default=False,
        help="Overwrite the destination if it exists.",
    )
    args = parser.parse_args()
    main(args.old, args.new, overwrite=args.overwrite)
