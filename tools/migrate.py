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
import os
import subprocess
import sys
from collections import OrderedDict


def main():
    parser = argparse.ArgumentParser(
        description="Migrate quizzes from old JSON format to new text format."
    )
    parser.add_argument("name")
    parser.add_argument(
        "-f", "--force", action="store_true", help="Overwrite old quiz."
    )
    parser.add_argument(
        "--stdout", action="store_true", help="Print v2 of quiz to standard output."
    )
    args = parser.parse_args()

    path = get_path(args.name)
    if path is None:
        warning(
            "No such quiz named '{}'; assuming that it is a file path instead.",
            args.name
        )
        path = args.name
    else:
        info("Detected quiz at {}.", path)

    try:
        with open(path, "r", encoding="utf-8") as f:
            quiz = json.load(f, object_hook=OrderedDict)
    except FileNotFoundError:
        error("Could not open '{}' for reading.", path)
    except IOError:
        error("Could not read from '{}' due to IO error.", path)

    if args.stdout:
        try:
            migrate(quiz, sys.stdout)
        except RuntimeError as e:
            error(str(e))
    else:
        if args.force:
            new_path = path
            info("Overwriting quiz at {} due to -f flag.", path)
        else:
            if get_path(args.name + "-v2") is not None:
                error("Quiz named '{}-v2' already exists. ", args.name)
            pathname, ext = os.path.splitext(path)
            new_path = pathname + "-v2" + ext
            info("Creating new quiz at {}.", new_path)

        try:
            with open(new_path, "w") as writer:
                migrate(quiz, writer)
        except FileNotFoundError:
            error("Could not open '{}' for writing.", path)
        except IOError:
            error("Could not write to '{}' due to IO error.", path)
        except RuntimeError as e:
            error(str(e))


def get_path(name):
    env = os.environ.copy()
    env["NO_COLOR"] = "yes"
    proc = subprocess.run(
        ["quiz", "path", name],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
    )
    path = proc.stdout.strip().decode("utf-8")
    if path.startswith(("Error", "error")):
        return None
    else:
        return path


def migrate(quiz, writer):
    default_kind = quiz.get("default_kind", "ShortAnswer")
    for i, question in enumerate(quiz["questions"]):
        writer.write("[{:0>6}]\n".format(i))
        kind = question.get("kind", default_kind)
        if "text" in question:
            writer.write("q = [")
            if isinstance(question["text"], str):
                writer.write(to_toml_str(question["text"]))
            else:
                for i, text in enumerate(question["text"]):
                    writer.write(to_toml_str(text))
                    if i != len(question["text"]) - 1:
                        writer.write(", ")
            writer.write("]\n")
        elif kind != "Flashcard":
            raise RuntimeError("question missing `text` field")

        if kind == "ShortAnswer":
            writer.write("a = ")
            write_answer(question["answer"], writer)
            writer.write("\n")
        elif kind == "ListAnswer" or kind == "OrderedListAnswer":
            writer.write("answers = [")
            for i, answer in enumerate(question["answer_list"]):
                write_answer(answer, writer)
                if i != len(question["answer_list"]) - 1:
                    writer.write(", ")
            writer.write("]\n")
            if kind == "OrderedListAnswer":
                writer.write("ordered = true\n")
        elif kind == "MultipleChoice":
            writer.write("a = ")
            write_answer(question["answer"], writer)
            writer.write("\n")
            writer.write("choices = [")
            writer.write(", ".join(map(to_toml_str, question["candidates"])))
            writer.write("]\n")
        elif kind == "Ungraded":
            raise RuntimeError("Ungraded questions are no longer supported")
        elif kind == "Flashcard":
            if question["side1"] == "q":
                raise RuntimeError("side 1 of a flashcard cannot be 'q'")

            writer.write(question["side1"])
            writer.write(": ")
            if isinstance(question["side2"], list):
                writer.write("/".join(map(escape_answer, question["side2"])))
            else:
                writer.write(question["side2"])
            writer.write("\n")
        else:
            raise RuntimeError("unknown kind: " + kind)

#         if "explanations" in question:
#             for variants, explanation in question["explanations"]:
#                 writer.write(
#                     "explain(" + ", ".join(map(escape_answer, variants)) + "): "
#                 )
#                 writer.write(explanation)
#                 writer.write("\n")

        if "tags" in question:
            writer.write("tags = [")
            writer.write(", ".join(map(to_toml_str, question["tags"])))
            writer.write("]\n")

        writer.write("\n")


def write_answer(answer, writer):
    writer.write("[")
    if isinstance(answer, str):
        writer.write(to_toml_str(answer))
    else:
        writer.write(", ".join(map(to_toml_str, answer)))
    writer.write("]")


def escape_answer(answer):
    return answer.replace("/", "\\/").replace(",", "\\,")


def to_toml_str(s):
    return '"' + s.replace('"', '\\"') + '"'


def info(msg, *args):
    print("\033[36m[INFO]\033[0m " + msg.format(*args))


def warning(msg, *args):
    print("\033[33m[WARNING]\033[0m " + msg.format(*args))


def error(msg, *args, exit_code=2):
    sys.stderr.write("\033[31m[ERROR]\033[0m " + msg.format(*args) + "\n")
    if exit_code is not None:
        sys.exit(exit_code)


if __name__ == "__main__":
    main()
