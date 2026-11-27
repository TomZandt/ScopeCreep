"""The app's Resolve helper (app/resolve/colour_setup_helper.py) never outlives the app (#256).

Linux has the kernel's parent-death signal; macOS has none, so the helper starts a watcher. The watcher is plain
POSIX, so these run on Linux and macOS (there is no fork on Windows, which has a job object instead)."""
import os
import signal
import subprocess
import sys
import textwrap
import time
import unittest

if sys.platform == "win32":
    raise unittest.SkipTest("Windows ties the helper to the app with a job object")

HELPER = os.path.join(os.path.dirname(__file__), "..", "..", "app", "resolve", "colour_setup_helper.py")

# Stands in for the app: starts the helper with the watcher on, tells the test the helper's pid, then waits.
APP = textwrap.dedent("""
    import subprocess, sys, time
    helper = subprocess.Popen([sys.executable, sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              env=dict(__import__("os").environ, SCOPECREEP_HELPER_WATCH_PARENT="1"))
    helper.stdout.readline()  # started
    print(helper.pid, flush=True)
    time.sleep(60)
""")


def alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    # a killed child waiting to be reaped is a zombie: not running
    state = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    return bool(state) and not state.startswith("Z")


def gone_within(pid, seconds):
    end = time.time() + seconds
    while time.time() < end:
        if not alive(pid):
            return True
        time.sleep(0.1)
    return False


class HelperNeverOutlivesTheApp(unittest.TestCase):
    def start_app(self):
        app = subprocess.Popen([sys.executable, "-c", APP, HELPER], stdout=subprocess.PIPE, text=True)
        self.addCleanup(lambda: (app.kill(), app.wait(), app.stdout.close()))
        helper_pid = int(app.stdout.readline())
        self.addCleanup(lambda: alive(helper_pid) and os.kill(helper_pid, signal.SIGKILL))
        return app, helper_pid

    def test_the_helper_exits_when_the_app_is_killed(self):
        app, helper_pid = self.start_app()
        self.assertTrue(alive(helper_pid))
        app.kill()
        app.wait()
        self.assertTrue(gone_within(helper_pid, 5), "the helper outlived the app")

    def test_a_helper_stuck_in_a_resolve_call_is_stopped_too(self):
        # A scripting call that holds the interpreter lock for good: a watcher thread would never run.
        app, helper_pid = self.start_app()
        os.kill(helper_pid, signal.SIGSTOP)  # frozen, as in a call that never returns
        app.kill()
        app.wait()
        self.assertTrue(gone_within(helper_pid, 5), "a frozen helper outlived the app")

    def test_the_helper_runs_on_while_the_app_does(self):
        app, helper_pid = self.start_app()
        time.sleep(1.5)
        self.assertTrue(alive(helper_pid))

    def test_the_watcher_does_not_hold_the_helpers_pipes(self):
        helper = subprocess.Popen([sys.executable, HELPER], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  env=dict(os.environ, SCOPECREEP_HELPER_WATCH_PARENT="1"))
        self.addCleanup(helper.kill)
        self.addCleanup(helper.stdout.close)
        helper.stdin.write(b"quit\n")
        helper.stdin.close()
        # Reads to the end of the pipe: it ends with the helper, not later when the watcher notices.
        t = time.time()
        output = helper.stdout.read()
        self.assertTrue(output.startswith(b"result=started"), output)
        self.assertLess(time.time() - t, 0.4)
        helper.wait(5)


if __name__ == "__main__":
    unittest.main()
