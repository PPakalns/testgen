#!/usr/bin/env python3

import os
import subprocess
import re
import shutil
import zipfile
import sys
from pathlib import Path

DOS2UNIX       = True
APPEND_NEWLINE = False
VALIDATOR      = Path("validator.cpp")
TEST_FOLDER    = Path("testi")
ZIP            = Path("../testi.zip")
POINT_FILE     = Path("../punkti.txt")

POINTS_PER_SUBTASK = [0, 2, 5, 12, 81]


def extract_test_files(zip_archive: Path, folder: Path):
    if folder.exists():
        shutil.rmtree(TEST_FOLDER)
    with zipfile.ZipFile(zip_archive.absolute()) as zip:
        zip.extractall(folder.absolute())
    if DOS2UNIX:
        for f in folder.iterdir():
            subprocess.run(["dos2unix", f]).check_returncode()


def compile_validator(validator :Path):
    subprocess.run(["g++", "-Wall", "-std=c++11", "-o", "validator", validator.absolute()])\
        .check_returncode()
    print("Validator compiled")


def parse_points(point_file: Path):
    # Parse following file
    #    01.  1 x 2p. =  2p.
    # 02-09.  8 x 2p. = 16p.
    # 10-19. 10 x 3p. = 30p.
    # 20-32. 13 x 4p. = 52p.
    with point_file.open() as f:
        content = [x.strip() for x in f.readlines()]
    points_per_group = dict()
    points_per_group[0] = 0
    for line in content:
        line_prefix = line[:line.index('.')]
        if line_prefix.find("-") == -1:
            a = b = 1
        else:
            a, b = [int(x) for x in line_prefix.split('-')]
        line_sub = line[line.index('.') + 1:line.index('=')].split('x')
        line_sub = list(map(str.strip, line_sub))
        points = line_sub[0] if 'p' in line_sub[0] else line_sub[1]
        points = int(points[:points.index('p')])
        for group in range(a, b+1):
            points_per_group[group] = points
    return points_per_group


def get_input_files(test_folder :Path):
    matcher = re.compile(r"\.i(\d+)([a-z]*)$")
    test_files = {}
    for file in test_folder.iterdir():
        if file.is_dir():
            continue
        file_name = file.name
        match = matcher.search(file_name)
        if match:
            group = int(match.group(1))
            if group not in test_files:
                test_files[group] = []
            test_files[group].append(file)
    test_files = {group: sorted(files) for group, files in test_files.items()}
    return test_files


def validate_test(test_file :Path, subtask):
    print(f"Validating\t{test_file}\t\tSubtask: {subtask}"),
    args = ["./validator"]
    if subtask is not None:
        args += ["--group", str(subtask)]
    args += ["<", str(test_file)]
    subprocess.run(" ".join(args), shell=True).check_returncode()


def get_subtask_assignment(subtask_groups, group_points):
    assigned_groups = set()
    subtask_assignment = []
    must_fail = None

    for subtask in range(len(subtask_groups)):
        groups = set()
        points_required = POINTS_PER_SUBTASK[subtask]

        for group in subtask_groups[subtask]:
            # Matching done
            if not points_required and groups:
                break
            # Group already assigned
            if group in assigned_groups:
                continue

            if group_points[group] <= points_required:
                # Assign group to subtask
                points_required -= group_points[group]
                assigned_groups.add(group)
                groups.add(group)

        if points_required or not groups:
            must_fail = f"Failed to assign groups for subtask #{subtask}"

        subtask_assignment.append((subtask, groups))
    return subtask_assignment, must_fail


def main():
    compile_validator(VALIDATOR)
    extract_test_files(ZIP, TEST_FOLDER)
    group_points = parse_points(POINT_FILE)
    tests = get_input_files(TEST_FOLDER)
    subtask_count = len(POINTS_PER_SUBTASK)

    # For each subtask find groups that matches the description
    subtask_groups = [set() for _ in range(subtask_count)]
    all_groups = set()

    for group, files in sorted(tests.items()):
        all_groups.add(group)
        passed_one = False

        subtask_range = range(2, len(POINTS_PER_SUBTASK)) if group > 1 else\
                            [0] if group == 0 else [1]

        for subtask in subtask_range:
            passed = True

            for file in files:
                try:
                    validate_test(file, subtask)
                except subprocess.CalledProcessError:
                    print(f"\t\t = = = = FAIL = = = = \n")
                    passed = False
                    break

            if passed:
                passed_one = True
                subtask_groups[subtask].add(group)
                print(f"\t\t OK \n")

        if not passed_one:
            raise f"Group {group} failed to pass validator"

    # Checking if groups can be assigned to subtasks to acquire
    # requested points for each subtask
    subtask_assignment, must_fail = get_subtask_assignment(subtask_groups, group_points)
    total_points = 0
    subtask_points = [0] * subtask_count
    unused_groups = all_groups.copy()

    for subtask, groups in subtask_assignment:
        group_stats = []
        for group in sorted(groups):
            unused_groups.remove(group)
            total_points += group_points[group]
            subtask_points[subtask] += group_points[group]
            group_stats += [f"{group} <=> {group_points[group]}p ({subtask_points[subtask]}p)"]
        print()
        print(f"For subtask {subtask} following groups are assigned")
        print("SIZE=", len(groups), "\t", ',  '.join(group_stats))
        print("SIZE=", len(subtask_groups[subtask]), "\tPossible groups were", subtask_groups[subtask])
        print("\tPoints per subtask:", subtask_points[subtask])

    print("")
    print(f"\tUnused groups {unused_groups}")
    print(f"\tExpeced points {POINTS_PER_SUBTASK}")
    print(f"\tGot points     {subtask_points}")
    if must_fail:
        print(must_fail)
        exit(1)
    assert(not unused_groups)
    assert(subtask_points == POINTS_PER_SUBTASK)
    assert(total_points == 100)
    print("GREAT SUCCESS")

if __name__ == "__main__":
    main()
