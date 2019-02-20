import yaml
import argparse
import asyncio
import utility

from colorama import Fore, Back, Style
from pathlib import Path
from typing import Union, List, Optional
from test_assignment import TestAssignment

class Unit:
    pass

class Task(Unit):
    def __init__(self, name: str, title: str, public_groups: List[int],
                 test_archive: Path, validator: Path, point_file: Path,
                 subtask_points: List[int]):
        self.name = name
        self.title = title
        self.public_groups = public_groups
        self.test_archive = test_archive
        self.validator = validator
        self.point_file = point_file
        self.subtask_points = subtask_points

    def print_summary(self):
        text = f"Task: {self.name}: {self.title}"
        width = len(text)
        print(Fore.LIGHTBLACK_EX + "#" * width)
        print(Fore.YELLOW, end="")
        print(text)
        print(Style.RESET_ALL, end="")
        print(f"\tPublic groups: {self.public_groups}")

class Contest(Unit):
    def __init__(self, name: str, description: str, tasks: List[Task]):
        self.name = name
        self.description = description
        self.tasks = tasks

    def print_summary(self):
        text = f"Contest: {self.name}: {self.description}"
        width = len(text)
        print("\n\n")
        print(Fore.BLUE + "=" * width)
        print(Fore.BLUE + "=" * width)
        print()
        print(Fore.YELLOW, end="")
        print(text)
        print(Style.RESET_ALL, end="")
        print(f"\tTask summary: {[task.name for task in self.tasks]}")
        print()
        print(Fore.BLUE + "-" * width)
        print(Style.RESET_ALL, end="")


def load_contest(config_path: Path) -> Contest:
    with config_path.open() as f:
        config = yaml.load(f)
    contest_dir = config_path.parent

    tasks = []
    for task_name, task_config in config['tasks'].items():
        task_yaml_path = contest_dir.joinpath(
            task_config.get('config', Path(task_name, 'task.yaml'))
        )
        tasks.append(load_task(task_yaml_path))

    return Contest(config['name'], config['description'], tasks)


def load_task(config_path: Path) -> Task:
    with config_path.open() as f:
        config = yaml.load(f)
    task_dir = config_path.parent

    public_groups = config.get('public_groups', [0, 1])
    test_archive = task_dir.joinpath(config.get('test_archive', 'testi.zip'))
    validator = task_dir.joinpath(config.get('validator', 'riki/validator.cpp'))
    point_file = task_dir.joinpath(config.get('point_file', 'punkti.txt'))
    subtask_points = config.get('subtask_points', [0, 2])
    return Task(config['name'], config['title'], public_groups, test_archive,
                validator, point_file, subtask_points)
