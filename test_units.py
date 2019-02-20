import asyncio
import utility
import shutil
import shlex
import re

from pathlib import Path
from typing import Iterable, List, Dict, Set

class Test:
    def __init__(self, tid: str, file: Path):
        self.tid = tid
        self.file = file

    async def validate(self, validator: Path, subtask: int):
        try:
            await utility.shell([shlex.quote(str(validator)), '--group', str(subtask), 
                                 '<', shlex.quote(str(self.file))])
            print(f"\t{self.file} : {subtask:3}  OK!")
            return True
        except utility.NonZeroReturnCode:
            print(f"\t{self.file} : {subtask:3}")
            return False


class TestGroup:
    def __init__(self, gid: int, points: int):
        self.gid = gid
        self.points = points
        self.tests: Dict[str, Test] = {}
        self.subtask_matches: Set[int] = set()

    def set_tests(self, files: Dict[str, Path]):
        self.tests = {tid: Test(tid, file) for tid, file in files.items()}

    async def match_subtasks(self, validator: Path, subtask_list: Iterable[int]):
        if not self.tests:
            raise Exception("No tests available")
        self.subtask_matches.clear()
        for subtask in subtask_list:
            add_match = True
            for test in self.tests.values():
                if not (await test.validate(validator, subtask)):
                    add_match = False
                    break
            if add_match:
                self.subtask_matches.add(subtask)


class Tests:
    def __init__(self, point_file: Path, test_dir: Path, public_groups: List[int]):
        assert(len(set(public_groups)) == len(public_groups))
        self.public_groups = public_groups
        self.groups = {gid: TestGroup(gid, points) for gid, points in read_points(point_file).items()}
        input_files = get_input_files(test_dir)
        for gid, tests in input_files.items():
            self.groups[gid].set_tests(tests)

    async def match_subtasks(self, validator: Path, subtask_list: Iterable[int]):
        for test_group in self.groups.values():
            await test_group.match_subtasks(validator, subtask_list)

    def print_summary(self):
        total_public_points = 0
        for pgid in self.public_groups:
            total_public_points += self.groups[pgid].points
        total_test_count = 0
        for group in self.groups.values():
            total_test_count += len(group.tests)
        print(f"\tTest group cnt: {len(self.groups)}")
        print(f"\tTotal test cnt: {total_test_count}")
        print(f"\tTotal public points: {total_public_points}")


def get_input_files(test_folder :Path) -> Dict[int, Dict[str, Path]]:
    test_files: Dict[int, Dict[str, Path]] = {}

    matcher = re.compile(r"\.(i|o)(\d+)([a-z]*)$")
    for file in test_folder.iterdir():
        if file.is_dir():
            raise Exception(f"Unexpected directory {file}")
        file_name = file.name
        match = matcher.search(file_name)
        if match:
            if match.group(1) == "o":
                continue
            group = int(match.group(2))
            if group not in test_files:
                test_files[group] = {}
            if match.group(3) in test_files[group]:
                raise Exception(f"Duplicated test group {match.group(3)}, {file}")
            test_files[group][match.group(3)] = file
        else:
            raise Exception(f"File doesn't match with regex {file}")
    return test_files


def read_points(point_file: Path) -> Dict[int, int]:
    print(f"Reading point file {point_file}")
    # Parse following file
    # 0-9 5 komentars
    # 10 - 15 5 komentars
    with point_file.open() as f:
        content = [x.strip() for x in f.readlines()]
    points_per_group: Dict[int, int] = dict()
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


async def extract_tests(test_zip: Path, target_dir: Path, dos2unix: bool):
    if target_dir.exists():
        shutil.rmtree(target_dir)
    target_dir.mkdir(parents=True)

    print(f"Extracting '{test_zip}' to '{target_dir}'")

    args = ['unzip', str(test_zip), '-d', str(target_dir)]
    await utility.run(args)

    if dos2unix:
        print("Applying dos2unix to all files in '{target_dir}'")
        for file in target_dir.iterdir():
            if file.is_dir():
                raise Exception("Bad path {file}")
            await utility.run(["dos2unix", str(file)])

