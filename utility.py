import asyncio

from pathlib import Path
from typing import List

class NonZeroReturnCode(Exception):
    pass


async def run(args: List[str]):
    proc = await asyncio.create_subprocess_exec(*args)
    await proc.wait()
    if proc.returncode != 0:
        raise NonZeroReturnCode(f"Failed to execute command '{args}'. Returned {proc.returncode}")


async def shell(args: List[str]):
    cmd = ' '.join(args)
    print(cmd)
    proc = await asyncio.create_subprocess_shell(cmd)
    await proc.wait()
    if proc.returncode != 0:
        raise NonZeroReturnCode(f"Failed to execute shell command '{args}'. Returned {proc.returncode}")


async def compile_validator(validator: Path, output: Path):
    print(f"Compiling validator {validator}")
    await run(["g++", "-Wall", "-std=c++17", "-o", str(output), str(validator)])

