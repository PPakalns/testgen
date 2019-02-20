import argparse
import utility
import asyncio

from colorama import Fore, Back, Style
from pathlib import Path
from task_units import Unit, Task, Contest
from test_assignment import TestAssignment
from typing import Optional, List, Union, cast
from test_units import extract_tests, Tests

class ValidationResult:
    def print_summary(self):
        raise NotImplementedError

class TaskValidationResult(ValidationResult):

    def __init__(self, task: Task):
        self.state = "unresolved"
        self.task: Task = task
        self.tests: Optional[Tests] = None
        self.test_assignment: Optional[TestAssignment] = None
        self.exception: Optional[Exception] = None

    def set_task(self, task: Task):
        self.task = task

    def set_tests(self, tests: Tests):
        self.tests = tests

    def set_test_assignment(self, test_assignment: TestAssignment):
        self.test_assignment = test_assignment

    def set_success(self):
        if self.state == "unresolved":
            self.state = "success"

    def set_fail(self, exception: Exception):
        self.state = "fail"
        self.exception = exception

    def print_summary(self):

        if self.task:
            self.task.print_summary()

        if self.tests:
            self.tests.print_summary()

        if self.test_assignment:
            self.test_assignment.print_summary()

        if self.success():
            print(Fore.GREEN + "GREAT SUCCESS")
        elif self.failed():
            print(Back.RED + "VALIDATION FAILED")
            print(self.exception)
        else:
            print(Back.BLUE + "VALIDATION NOT FINISHED")
        print(Style.RESET_ALL)

    def success(self):
        return not self.failed() and self.state == "success"

    def failed(self):
        return self.state == "fail"


class ContestValidationResult(ValidationResult):
    def __init__(self, contest: Contest, task_validation_results: List[TaskValidationResult]):
        self.contest = contest
        self.task_validation_results = task_validation_results

    def print_summary(self):

        print("\n")
        self.contest.print_summary()

        for task_result in self.task_validation_results:
            task_result.print_summary()


async def validate_task(task: Task, opts: argparse.Namespace) -> TaskValidationResult:
    validation_result = TaskValidationResult(task)

    try:
        test_dir = Path('testi_validator',  task.name)
        if opts.extract:
            await extract_tests(task.test_archive, test_dir, opts.dos2unix)
        tests = Tests(task.point_file, test_dir, task.public_groups)
        validation_result.set_tests(tests)

        compiled_validator = Path('testi_validator', f'validator{task.name}')

        await utility.compile_validator(task.validator, compiled_validator)

        await tests.match_subtasks(compiled_validator,
                                   range(0, len(task.subtask_points)))

        assignment = TestAssignment(task.subtask_points, tests)

        validation_result.set_test_assignment(assignment)

        assignment.validate()

        validation_result.set_success()
    except Exception as e:
        validation_result.set_fail(e)

    return validation_result


async def validate(obj: Union[Task, Contest], opts: argparse.Namespace) -> ValidationResult:
    if type(obj) is Contest:
        contest = cast(Contest, obj)
        task_validation_results = list(await asyncio.gather(*(validate_task(task, opts) for task in contest.tasks)))
        return ContestValidationResult(contest, task_validation_results)
    else:
        task = cast(Task, obj)
        return await validate_task(task, opts)

