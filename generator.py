import os
import shutil
import tempfile
import subprocess
from pathlib import Path

def compile(source:Path, output:Path):
    print(f"Compiling {source} to {output}")
    subprocess.run(["g++", "-Wall", "-std=c++14", "-o", output.absolute(), source.absolute()])\
        .check_returncode()

class TestGen:

    def __init__(self, filename, generator, solution, outputDir):
        self.filename = filename

        self.outputDir = outputDir
        if os.path.exists(self.outputDir):
            shutil.rmtree(self.outputDir)
        os.mkdir(self.outputDir)

        self.tempDir = Path(tempfile.mkdtemp())

        self.generator = Path(self.tempDir, "generator")
        self.solution = Path(self.tempDir, "solution")
        compile(Path(generator), self.generator)
        compile(Path(solution), self.solution)

        self.test_group = -1
        self.test_in_group = 0
        self.group_list = []

    def End(self):
        print("Summary:")
        cnt = -1
        points = 0
        for ginfo in self.group_list:
            cnt += 1
            points += ginfo[0]
            print(f"\tGroup {cnt:02}: {ginfo[0]} {points:3}\t{ginfo[1]}")
        print(f"TOTAL POINTS: {points}")
        assert(points == 100)
        shutil.rmtree(self.tempDir)

    def NewGroup(self, points, comment = ""):
        if comment is None:
            comment = self.group_list[-1][1] # Previous comment
        self.test_group += 1
        self.test_in_group = 0
        self.group_list.append((points, comment))
        print(f"\nGroup {self.test_group}\n")

    def IncreaseTest(self):
        self.test_in_group += 1

    def GetExtension(self, input):
        ioLetter = 'i' if input else 'o'
        letter = chr(self.test_in_group + ord('a'))
        return f".{ioLetter}{self.test_group:02}{letter}"

    def GetInputFile(self):
        return Path(self.outputDir, self.filename + self.GetExtension(True))

    def GetOutputFile(self):
        return Path(self.outputDir, self.filename + self.GetExtension(False))

    def GenerateAnswer(self, input:Path, output:Path):
        print(f"Generating answer {output}")
        with input.open('r') as finp:
            with output.open('w') as fout:
                subprocess.run([self.solution.absolute()], stdin = finp, stdout = fout)\
                    .check_returncode()

    def GenerateTest(self, args):
        args = [str(arg) for arg in args]
        input = self.GetInputFile()
        print(f"Generating test {input}")
        output = self.GetOutputFile()
        with input.open('w') as finp:
            subprocess.run([str(self.generator)] + args, stdout = finp)\
                .check_returncode()
        self.GenerateAnswer(input, output)
        self.IncreaseTest()

    def GenerateRawTest(self, rawFile):
        input = self.GetInputFile()
        print(f"Raw test {input}")
        output = self.GetOutputFile()
        input.write_text(rawFile)
        self.GenerateAnswer(input, output)
        self.IncreaseTest()

    def GeneratePointFile(self, pointFilePath:Path):
        lines = [] # (sgroup, egroup, points, comments)
        group_count = -1
        for gr in self.group_list:
            group_count += 1
            if lines and lines[-1][2:] == gr:
                lines[-1] = (lines[-1][0], group_count, gr[0], gr[1])
            else:
                lines.append((group_count, group_count, gr[0], gr[1]))

        with pointFilePath.open('w') as f:
            for l in lines:
                print(f"{l[0]}-{l[1]} {l[2]}{' ' if l[3] else ''}{l[3]}", file=f)



