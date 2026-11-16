import unittest

from scopecreep_harness import projects


class FakeProject:
    def __init__(self, name):
        self.name = name

    def GetName(self):
        return self.name


class FakePM:
    """A Project Manager with a root and one level of folders."""

    def __init__(self, root=(), folders=None, current=None):
        self.tree = {"": list(root)}
        for f, names in (folders or {}).items():
            self.tree[f] = list(names)
        self.folder = ""
        self.current = FakeProject(current) if current else None

    def GotoRootFolder(self):
        self.folder = ""
        return True

    def GetFolderListInCurrentFolder(self):
        return [f for f in self.tree if f] if self.folder == "" else []

    def CreateFolder(self, name):
        self.tree[name] = []
        return True

    def OpenFolder(self, name):
        if name not in self.tree:
            return False
        self.folder = name
        return True

    def GetProjectListInCurrentFolder(self):
        return list(self.tree[self.folder])

    def LoadProject(self, name):
        if name not in self.tree[self.folder]:
            return None
        self.current = FakeProject(name)
        return self.current

    def GetCurrentProject(self):
        return self.current

    def CreateProject(self, name):
        self.tree[self.folder].append(name)
        self.current = FakeProject(name)
        return self.current

    def DeleteProject(self, name):
        if name not in self.tree[self.folder]:
            return False
        self.tree[self.folder].remove(name)
        return True

    def SaveProject(self):
        return True


class ProjectFolderTest(unittest.TestCase):
    def test_test_projects_open_from_the_scopecreep_folder(self):
        pm = FakePM(root=["mine"], folders={"scopecreep": ["scopecreep-test-dwg-rec709"]})
        self.assertIsNotNone(projects.load(pm, "scopecreep-test-dwg-rec709"))
        self.assertEqual(pm.folder, "scopecreep")

    def test_the_users_project_reopens_from_the_root(self):
        pm = FakePM(root=["mine"], folders={"scopecreep": []}, current="scopecreep-test-x")
        pm.folder = "scopecreep"
        self.assertIsNotNone(projects.load(pm, "mine"))
        self.assertEqual(pm.folder, "")

    def test_the_folder_is_created_when_missing(self):
        pm = FakePM(root=["mine"])
        projects.enter_folder(pm)
        self.assertEqual(pm.folder, "scopecreep")

    def test_new_test_projects_are_made_in_the_folder_not_beside_the_users(self):
        pm = FakePM(root=["mine"], current="mine")
        projects._ensure_scratch(pm)
        self.assertEqual(pm.tree[""], ["mine"])
        self.assertEqual(pm.tree["scopecreep"], [projects.SCRATCH])

    def test_clean_deletes_only_the_folders_test_projects_and_reopens_the_users(self):
        pm = FakePM(root=["mine", "scopecreep-test-stray-at-root"],
                    folders={"scopecreep": ["scopecreep-test-a", projects.SCRATCH]}, current="mine")
        gone = projects.clean(pm, "mine")
        self.assertEqual(sorted(gone), sorted(["scopecreep-test-a", projects.SCRATCH]))
        self.assertEqual(pm.tree["scopecreep"], [])
        self.assertEqual(pm.tree[""], ["mine", "scopecreep-test-stray-at-root"])
        self.assertEqual(pm.current.GetName(), "mine")


if __name__ == "__main__":
    unittest.main()
