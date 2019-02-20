#!/usr/bin/env python3

import argparse
import yaml
import asyncio
from pathlib import Path
from colorama import init

from typing import List
from test_validation import validate
from task_units import Unit, Contest, Task, load_contest, load_task

async def multiple_tasks(tasks):
    return await asyncio.gather(*tasks, return_exceptions=True)


def main(opts: argparse.Namespace):

    evaluate = []
    for config in opts.config:
        config_path = Path(config)

        with config_path.open() as f:
            config = yaml.load(f)

        # Contest configuration has "tasks" configuration
        if "tasks" in config:
            loaded_contest = load_contest(config_path)
            evaluate.append(validate(loaded_contest, opts))
        else:
            loaded_task = load_task(config_path)
            evaluate.append(validate(loaded_task, opts))

    loop = asyncio.get_event_loop()
    results = loop.run_until_complete(multiple_tasks(evaluate))

    for result in results:
        result.print_summary()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--dos2unix", action="store_true")
    parser.add_argument("--use-extracted", dest="extract", action="store_false", help="Use tests from folder, do not extract from zip.")
    parser.add_argument(nargs="+", dest="config", type=str, help="Yaml file which defining contest or task")
    opts = parser.parse_args()
    init()
    main(opts)


