#!/usr/bin/env python3

import os
import subprocess
import re
import shutil
import zipfile
import sys
import argparse
from pathlib import Path

def extract_test_files(zip_archive: Path, folder: Path, dos2unix):
    if folder.exists():
        shutil.rmtree(str(folder.absolute()))
    with zipfile.ZipFile(zip_archive.absolute()) as zip:
        zip.extractall(folder.absolute())
    if dos2unix:
        for f in folder.iterdir():
            subprocess.run(["dos2unix", f]).check_returncode()


def compile_validator(validator :Path):
    subprocess.run(["g++", "-Wall", "-std=c++17", "-o", "validator", str(validator.absolute())])\
        .check_returncode()
    print("Validator compiled")


def parse_points(point_file: Path):
    # Parse following file
    # 0-9 5 komentars
    # 10 - 15 5 komentars
    with point_file.open() as f:
        content = [x.strip() for x in f.readlines()]
    points_per_group = dict()
    maxGroup = 0
    for line in content:
        vars = line.replace("-", " ").split()
        a = int(vars[0])
        b = int(vars[1])
        points = int(vars[2])
        for group in range(a, b+1):
            assert(group not in points_per_group)
            points_per_group[group] = points
        maxGroup = b
    if 0 not in points_per_group:
        points_per_group[0] = 0
    for i in range(0, maxGroup + 1):
        if i not in points_per_group:
            raise Exception(f"Point file is not continious. Check group {i}")
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


def get_subtask_assignment(subtask_groups, group_points, points_per_subtask):
    assigned_groups = set()
    subtask_assignment = []
    must_fail = None

    for subtask in range(len(subtask_groups)):
        groups = set()
        points_required = points_per_subtask[subtask]

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


def main(opts):
    compile_validator(Path(opts.validator))
    if opts.extract:
        extract_test_files(Path(opts.zip), Path(opts.test_dir), opts.dos2unix)
    else:
        print("Zip extracting skipped.")

    group_points = parse_points(Path(opts.point_file))
    tests = get_input_files(Path(opts.test_dir))
    subtask_count = len(opts.points_per_subtask)

    # For each subtask find groups that matches the description
    subtask_groups = [set() for _ in range(subtask_count)]
    all_groups = set()

    for group, files in sorted(tests.items()):
        all_groups.add(group)
        passed_one = False

        subtask_range = range(2, len(opts.points_per_subtask)) if group > 1 else\
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
    subtask_assignment, must_fail = get_subtask_assignment(subtask_groups, group_points, opts.points_per_subtask)
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
    print(f"\tExpeced points {opts.points_per_subtask}")
    print(f"\tGot points     {subtask_points}")
    print(f"\n");

    # Some beutiful table
    print(f"{'Ap.uzd.':7} " + ''.join([str(x % 10) for x in range(len(all_groups))]))
    for subtask, groups in subtask_assignment:
        output = ""
        gr_list = [' '] * len(all_groups)
        for group in subtask_groups[subtask]:
            gr_list[group] = '░'
        for group in groups:
            gr_list[group] = '▓'
        print(f"{subtask:7} " + ''.join(gr_list))

    if must_fail:
        print(must_fail)
        exit(1)
    assert(not unused_groups)
    assert(subtask_points == opts.points_per_subtask)
    assert(total_points == 100)
    print("GREAT SUCCESS")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--dos2unix", action="store_true")
    parser.add_argument("--test_dir", default="testi")
    parser.add_argument("--use_extracted", dest="extract", action="store_false", help="Use tests from folder, do not extract from zip.")
    parser.add_argument("zip")
    parser.add_argument("validator")
    parser.add_argument("point_file")
    parser.add_argument('-g','--group', type=int, dest="points_per_subtask", action='append', help='Pievienot apakšuzdevumu ar punktiem', required=True)
    opts = parser.parse_args()
    main(opts)

