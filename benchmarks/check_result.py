#!/usr/bin/env python3

# https://gitlab.com/Nils-TUD/m3bench/-/blob/asplos22-ae/tools/check_result.py

import re
import sys

class PerfResult:
    def __init__(self, name, time, unit, variance, runs):
        self.name = name
        self.time = time
        self.unit = unit
        self.variance = variance
        self.runs = runs

    def __repr__(self):
        return "PERF[{}] = {} {} ({} with {} runs)\n".format(self.name, self.time, self.unit, self.variance, self.runs)

class TestResult:
    def __init__(self, name, desc):
        self.name = name
        self.desc = desc

    def __repr__(self):
        if self.name == "":
            return self.desc
        return self.name + ": " + self.desc

class Result:
    def __init__(self):
        self.failed_tests = 0
        self.succ_tests = 0
        self.failures = []
        self.perfs = {}

    def add_failed_test(self, name, desc):
        self.failures.append(TestResult(name, desc))

    def add_perf(self, pmatch):
        name = re.sub(r"^.*/([^/]+)$", r"\1", pmatch.group(1)) + ": " + pmatch.group(3)
        self.perfs[name] = PerfResult(name,
                                      float(pmatch.group(4)),
                                      pmatch.group(5),
                                      float(pmatch.group(6)),
                                      int(pmatch.group(7)))

    def __repr__(self):
        str = "{} / {} succeeded".format(self.failed_tests, self.succ_tests + self.failed_tests)
        if len(self.perfs) > 0:
            str += "\n"
            for p in self.perfs:
                str += "  " + repr(self.perfs[p])
        return str

re_test   = re.compile('^Testing "(.*?)" in (.*?):$')
re_failed = re.compile('^!\s+([^:]+):(\d+)\s+(.*?) FAILED$')
re_perf   = re.compile('^.*!\s+([^:]+):(\d+)\s+PERF\s+"(.*?)": ([\d\.]+) (\S+?) \(\+/\- ([0-9\-\.]+) with (\d+) runs\)$')
re_shdn   = re.compile('^.*\[(PE0:\S+\s*@\s*\d+|\S+\s*@\d+)\].*Shutting down$')
re_fsck   = re.compile('^.*(m3fsck:.*)$')
re_exit   = re.compile('^.*Child .*? exited with exitcode \d+$')
re_panic  = re.compile('^.*PANIC at(.*)$')

def parse_output(file):
    failed_asserts = 0
    res = Result()
    seen_shutdown = False
    seen_fsck = ''
    with open(file, 'r', errors='replace') as reader:
        line = reader.readline()
        test = ""
        while line != '':
            line = line.strip()
            # special handling for the TCU abort test
            if line.startswith("info: "):
                line = line[6:]
            tmatch = re_test.match(line)
            if tmatch:
                if test != "":
                    if failed_asserts == 0:
                        res.succ_tests += 1
                    else:
                        res.failed_tests += 1
                    failed_asserts = 0
                test = tmatch.group(1)
            else:
                fmatch = re_failed.match(line)
                if fmatch:
                    res.add_failed_test(fmatch.group(1) + ":" + fmatch.group(2), fmatch.group(3))
                    failed_asserts += 1
                else:
                    pmatch = re_perf.match(line)
                    if pmatch:
                        res.add_perf(pmatch)
                        res.succ_tests += 1
                    elif re_shdn.match(line):
                        seen_shutdown = True
                    elif re_exit.match(line):
                        res.failed_tests += 1
                        res.add_failed_test("", line)
                    elif re_fsck.match(line):
                        seen_fsck = re_fsck.match(line).group(1)
                    else:
                        panic_match = re_panic.match(line)
                        if panic_match:
                            res.add_failed_test("", "PANIC at " + panic_match.group(1))
                            res.failed_tests += 1

            line = reader.readline()
    if not seen_shutdown:
        res.failed_tests += 1
        res.add_failed_test("", "Test did not complete (no kernel shutdown)")
    if seen_fsck != '':
        res.failed_tests += 1
        res.add_failed_test("", seen_fsck)
    return res

if __name__ == '__main__':
    if len(sys.argv) != 2:
        print("Usage: {} <file>".format(sys.argv[0]))
        sys.exit(1)

    res = parse_output(sys.argv[1])
    for failed in res.failures:
        print("  {} \033[1mfailed\033[0m".format(failed), file=sys.stderr)

    sys.exit(0 if res.failed_tests == 0 else 1)
