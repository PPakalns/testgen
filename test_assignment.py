
from test_units import TestGroup, Tests
from typing import Dict, List, Union, Set

class TestAssignment:

    def __init__(self, subtask_points: List[int], tests: Tests):
        self.subtask_points = subtask_points
        self.tests = tests
        self.assigned_groups: Dict[int, Union[None, int]] = {gid: None for gid in self.tests.groups.keys()}

        self.assign_groups()

    def assign_groups(self):
        for subtask_id in range(len(self.subtask_points)):
            print(f"Processing subtask {subtask_id}")

            points_needed = self.subtask_points[subtask_id]

            for gid in sorted(self.assigned_groups.keys()):
                if self.assigned_groups[gid] is not None:
                    continue
                if subtask_id not in self.tests.groups[gid].subtask_matches:
                    continue

                group: TestGroup = self.tests.groups[gid]

                if group.points > points_needed:
                    continue

                points_needed -= group.points
                self.assigned_groups[gid] = subtask_id

    def get_summary(self):
        points_assigned = 0
        unused_groups: Set[int] = set()
        subtask_assigned_points = [0 for x  in range(len(self.subtask_points))]
        subtask_group_assignment = [set() for _ in self.subtask_points]
        for gid in self.assigned_groups.keys():
            subtask_id = self.assigned_groups[gid]
            if subtask_id is None:
                unused_groups.add(gid)
                continue
            group = self.tests.groups[gid]
            subtask_assigned_points[subtask_id] += group.points
            subtask_group_assignment[subtask_id].add(gid)
            points_assigned += group.points
        total_points = sum(self.subtask_points)
        return {
            'points_assigned': points_assigned,
            'unused_groups': unused_groups,
            'subtask_assigned_points': subtask_assigned_points,
            'subtask_group_assignment': subtask_group_assignment,
            'total_points': total_points,
        }


    def validate(self):
        summary = self.get_summary()
        errors = []
        if summary['points_assigned'] != 100:
            errors.append(f"Bad assignment {summary['points_assigned']}/100;")
        if summary['unused_groups']:
            errors.append(f"Unused groups {summary['unused_groups']};")
        if summary['subtask_assigned_points'] != self.subtask_points:
            errors.append(f"Incorrectly assigned points Expected: {self.subtask_points}," +
                          f"Got: {summary['subtask_assigned_points']};")
        if summary['total_points'] != 100:
            errors.append(f"Total points {summary['total_points']} != 100;")

        if errors:
            errors.insert(0, "ASSIGNMENT FAIL:")
            raise Exception(' '.join(errors))

    def print_summary(self):
        summary = self.get_summary()

        print("")
        print(f"\tUnused groups {summary['unused_groups']}")
        print(f"\tExpected points {self.subtask_points}")
        print(f"\tPoints per subtask {summary['subtask_assigned_points']}")
        print(f"\n");

        # Some beutiful table
        pubgr = [' '] * len(self.tests.groups)
        for public_group in self.tests.public_groups:
            pubgr[public_group] = 'X'
        print(f"{'Pub.gr.':7} " + ''.join(pubgr))
        print(f"{'Ap.uzd.':7} " + ''.join([str(x % 10) for x in range(len(self.tests.groups))]))
        for subtask_id in range(len(self.subtask_points)):
            output = ""
            gr_list = [' '] * len(self.tests.groups)
            for public_group in self.tests.public_groups:
                gr_list[public_group] = '│'
            for group in self.tests.groups.values():
                if subtask_id in group.subtask_matches:
                    gr_list[group.gid] = '░'
            for gid in summary['subtask_group_assignment'][subtask_id]:
                gr_list[gid] = '▓'
            print(f"{subtask_id:7} " + ''.join(gr_list))

        print(f"Points {summary['points_assigned']} / {summary['total_points']}")

