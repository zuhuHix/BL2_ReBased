"""convert_subset_rank_deficient (the lootables import's map check) on synthetic skeletons: every number is invented."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import slice_npc_assets as npc  # noqa: E402


def md5mesh(bones):
    rows = [f'\t"{name}" {parent} ( {x} {y} {z} ) ( 0 0 0 )' for name, parent, (x, y, z) in bones]
    return 'MD5Version 10\ncommandline ""\n\nnumJoints %d\nnumMeshes 0\n\njoints {\n%s\n}\n' % (len(bones), '\n'.join(rows))


def md5anim(names, frames):
    hierarchy = '\n'.join(f'\t"{name}" -1 63 0' for name in names)
    body = '\n\n'.join('frame %d {\n%s\n}' % (i, '\n'.join(' '.join(str(v) for v in row) for row in frame))
                       for i, frame in enumerate(frames))
    return f'MD5Version 10\nnumFrames {len(frames)}\nnumJoints {len(names)}\nframeRate 30\n\nhierarchy {{\n{hierarchy}\n}}\n\n{body}\n'


def reference(bones, flip=-1.0):
    """The UE pose of bones: flip -1 is the expected Y mirror of the MD5 positions, +1 is a wrong (unmirrored) map."""
    return [{'name': name, 'loc': [x, flip * y, z], 'quat': [0, 0, 0, 1]} for name, _, (x, y, z) in bones]


# A root and a lid, as the prop skeletons have: too few bones to determine a 3D map.
TWO_BONES = [('Root', -1, (0.0, 0.0, 0.0)), ('Lid', 0, (10.0, 20.0, 30.0))]
# Five bones spanning all three axes: the fit is determined.
FIVE_BONES = [('Root', -1, (0.0, 0.0, 0.0)), ('A', 0, (10.0, 0.0, 0.0)), ('B', 0, (0.0, 12.0, 0.0)),
              ('C', 0, (0.0, 0.0, 14.0)), ('D', 0, (5.0, 6.0, 7.0))]


class RankDeficientMapTest(unittest.TestCase):
    def convert(self, bones, flip=-1.0, function=None):
        logged = []
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'mesh.md5mesh').write_text(md5mesh(bones), encoding='utf-8')
            (root / 'ref.json').write_text(json.dumps(reference(bones, flip)), encoding='utf-8')
            names = [bone[0] for bone in bones]
            frames = [[[*bone[2], 0, 0, 0] for bone in bones]]
            (root / 'Open.md5anim').write_text(md5anim(names, frames), encoding='utf-8')
            convert = function or (lambda m, r, c: npc.convert_subset_rank_deficient(m, r, c, log=logged.append))
            output, report = convert(root / 'mesh.md5mesh', root / 'ref.json', {'Open': root / 'Open.md5anim'})
        return output, report, logged

    def test_two_bones_on_the_mirror_convert_and_log_per_clip(self):
        output, report, logged = self.convert(TWO_BONES)
        self.assertEqual(report['rank'], 2)
        self.assertTrue(report['mirror_tested_directly'])
        self.assertEqual(len(logged), 1)
        self.assertIn('rank-deficient: mirror tested directly on 2 bones', logged[0])
        self.assertEqual(output['Open']['frames'], 1)

    def test_two_bones_with_a_wrong_mirror_are_refused(self):
        with self.assertRaisesRegex(ValueError, 'not the expected Y mirror'):
            self.convert(TWO_BONES, flip=1.0)

    def test_the_bind_pose_comes_back_as_the_reference_pose(self):
        # Frame 0 is the bind pose, so the lid's local track must be the reference's local position.
        output, _, _ = self.convert(TWO_BONES)
        self.assertAlmostEqual(output['Open']['tracks']['Lid']['pos'][0][1], -20.0, places=4)

    def test_a_full_rank_skeleton_takes_the_usual_fit(self):
        _, report, logged = self.convert(FIVE_BONES)
        self.assertNotIn('mirror_tested_directly', report)
        self.assertEqual(logged, [])

    def test_a_full_rank_wrong_mirror_is_refused_by_the_fit(self):
        with self.assertRaisesRegex(ValueError, 'MD5 to UE map is not the expected Y mirror'):
            self.convert(FIVE_BONES, flip=1.0)

    def test_the_usual_entry_point_still_refuses_two_bones(self):
        # Characters and NPCs keep the exact check: convert_subset itself does not accept a small skeleton.
        with self.assertRaisesRegex(ValueError, 'MD5 to UE map is not the expected Y mirror'):
            self.convert(TWO_BONES, function=npc.convert_subset)


if __name__ == '__main__':
    unittest.main()
