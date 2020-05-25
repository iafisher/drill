"""Pre-commit configuration for git.

This file was created by precommit (https://github.com/iafisher/precommit).
You are welcome to edit it yourself to customize your pre-commit hook.
"""
from precommitlib import checks


def init(precommit):
    precommit.check(checks.NoStagedAndUnstagedChanges())
    precommit.check(checks.NoWhitespaceInFilePath())
    precommit.check(checks.DoNotSubmit())

    precommit.check(checks.Command("E2ETests", ["./t"], slow=True))

    # Check Python format with black.
    precommit.check(checks.PythonFormat())

    # Lint Python code with flake8.
    precommit.check(checks.PythonLint())

    # Check the order of Python imports with isort.
    precommit.check(checks.PythonImportOrder())

    # Check Python static type annotations with mypy.
    # precommit.check(checks.PythonTypes())

    # Lint JavaScript code with ESLint.
    precommit.check(checks.JavaScriptLint())
