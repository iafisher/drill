#!/usr/bin/env python3
"""
Use this script to migrate quizzes from the old JSON format of version 1 to the textual
format of version 2.

Note that ungraded questions and dependencies are no longer supported in version 2. Some
niche use cases (like having "q" as the first side of a flashcard) may no longer work.

Author:  Ian Fisher (iafisher@protonmail.com)
Version: September 2019
"""
import argparse
import json
import sys
from collections import OrderedDict


def migrate(quiz, writer):
    default_kind = quiz.get("default_kind", "ShortAnswer")
    for question in quiz["questions"]:
        kind = question.get("kind", default_kind)
        if "text" in question:
            if isinstance(question["text"], str):
                writer.write("q: ")
                writer.write(question["text"])
                writer.write("\n")
            else:
                for text in question["text"]:
                    writer.write("q: ")
                    writer.write(text)
                    writer.write("\n")
        elif kind != "Flashcard":
            raise RuntimeError("question missing `text` field")

        if kind == "ShortAnswer":
            writer.write("a: ")
            write_answer(question["answer"], writer)
            writer.write("\n")
        elif kind == "ListAnswer":
            writer.write("a: ")
            for i, answer in enumerate(question["answer_list"]):
                write_answer(answer, writer)
                if i != len(question["answer_list"]) - 1:
                    writer.write(", ")
            writer.write("\n")
        elif kind == "OrderedListAnswer":
            writer.write("a: ")
            for i, answer in enumerate(question["answer_list"]):
                write_answer(answer, writer)
                if i != len(question["answer_list"]) - 1:
                    writer.write(", ")
            writer.write("\n")
            writer.write("- ordered: true\n")
        elif kind == "MultipleChoice":
            writer.write("a: ")
            write_answer(question["answer"], writer)
            writer.write("\n")
            writer.write("choices: ")
            writer.write(",".join(map(escape_answer, question["candidates"])))
            writer.write("\n")
        elif kind == "Ungraded":
            raise RuntimeError("Ungraded questions are no longer supported")
        elif kind == "Flashcard":
            if question["side1"] == "q":
                raise RuntimeError("side 1 of a flashcard cannot be 'q'")

            writer.write(question["side1"])
            writer.write(": ")
            if isinstance(question["side2"], list):
                writer.write(",".join(map(escape_answer, question["side2"])))
            else:
                writer.write(question["side2"])
            writer.write("\n")
        else:
            raise RuntimeError("unknown kind: " + kind)

        if "explanations" in question:
            for variants, explanation in question["explanations"]:
                writer.write("explain(" + ", ".join(map(escape_answer, variants)) + "): ")
                writer.write(explanation)
                writer.write("\n")

        if "tags" in question:
            writer.write("- tags: ")
            writer.write(", ".join(question["tags"]))
            writer.write("\n")

        writer.write("\n")


def write_answer(answer, writer):
    if isinstance(answer, str):
        writer.write(escape_answer(answer))
    else:
        writer.write("/".join(map(escape_answer, answer)))


def escape_answer(answer):
    return answer.replace("/", "\\/").replace(",", "\\,")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Migrate quizzes from old JSON format to new text format."
    )
    parser.add_argument("path_to_quiz")
    parser.add_argument(
        "-f", "--force", action="store_true", help="Overwrite old quiz."
    )
    args = parser.parse_args()

    with open(args.path_to_quiz, "r", encoding="utf-8") as f:
        quiz = json.load(f, object_hook=OrderedDict)

    if args.force:
        with open(args.path_to_quiz, "w") as writer:
            migrate(quiz, writer)
    else:
        migrate(quiz, sys.stdout)
