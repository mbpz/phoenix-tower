"""Offline checks for the small, portable handoff package (no Blender required)."""
import hashlib
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
HANDOFF = ROOT / 'docs/handoff'


class HandoffAssetsTests(unittest.TestCase):
    def test_manifest_files_are_portable_and_match_hashes(self):
        manifest = json.loads((HANDOFF / 'manifest.json').read_text(encoding='utf-8'))
        paths = []
        for entry in manifest['files']:
            path = Path(entry['path'])
            self.assertFalse(path.is_absolute())
            self.assertNotIn('..', path.parts)
            self.assertTrue(path.as_posix().startswith('docs/handoff/'))
            data = (ROOT / path).read_bytes()
            self.assertEqual(len(data), entry['bytes'], str(path))
            self.assertEqual(hashlib.sha256(data).hexdigest(), entry['sha256'], str(path))
            paths.append(path.as_posix())
        self.assertEqual(len(paths), len(set(paths)))
        self.assertIn('docs/handoff/yellow-crane/exterior-whitebox-08-portable.blend', paths)

    def test_v13_evidence_keeps_bounded_verification_distinct_from_visual_acceptance(self):
        evidence = HANDOFF / 'yellow-crane/evidence'
        def read(suffix):
            return json.loads((evidence / f'exterior-whitebox-13-{suffix}.json').read_text())
        geometry, disk, final, visual = (read(s) for s in
                                       ('readback', 'disk-readback', 'final-verification', 'visual-verdict'))
        self.assertEqual(geometry['v13_file_sha256'], disk['source_file_sha256'])
        self.assertEqual(final['v13_source_sha256'], disk['source_file_sha256'])
        self.assertTrue(disk['scene_geometry_matches_live'])
        self.assertEqual(disk['meshes'], 34)
        self.assertEqual(set(geometry['changed_meshes']), {'Fifth_lower_canopy', 'L5_support_PROXY'})
        self.assertEqual(len(geometry['unchanged_meshes']), 32)
        self.assertEqual(len(geometry['endpoint_checks']), 40)
        tips = [p for p in geometry['endpoint_checks'] if p['is_tip']]
        self.assertEqual(len(tips), 12)
        for point in geometry['endpoint_checks']:
            target = 38.8 if point['is_tip'] else 37.02
            self.assertLessEqual(abs(point['actual_xyz_m'][2]-target), 3e-6)
        self.assertEqual(len(geometry['clearance_checks']), 20)
        for support in geometry['clearance_checks']:
            self.assertGreaterEqual(support['vertical_clearance_m'], .02-3e-6)
            self.assertGreater(support['support_height_m'], .04)
        self.assertTrue(read('post-verification')['duplicate_version_rejected'])
        self.assertFalse(final['export_to_game'])
        self.assertEqual(visual['verdict'], 'revise')
        self.assertLess(visual['score'], 90)

    def test_v14_evidence_keeps_endpoint_fix_and_unchecked_posts_bounded(self):
        evidence = HANDOFF / 'yellow-crane/evidence'
        def read(suffix):
            return json.loads((evidence / f'exterior-whitebox-14-{suffix}.json').read_text())
        geometry, final, post, visual = (read(s) for s in
                                      ('readback', 'final-verification', 'post-verification', 'visual-verdict'))
        self.assertEqual(set(geometry['changed_meshes']), {'Ground_canopy', 'Third_canopy'})
        self.assertEqual(len(geometry['unchanged_meshes']), 32)
        self.assertEqual(len(geometry['endpoint_checks']), 120)
        for label, target in final['endpoint_constraints_m'].items():
            points = [p for p in geometry['endpoint_checks'] if p['canopy'] == label]
            self.assertEqual(len(points), 40)
            self.assertEqual(sum(p['is_tip'] for p in points), 12)
            for point in points:
                self.assertLessEqual(abs(point['actual_z_m'] - target['tip' if point['is_tip'] else 'low']), 3e-6)
        self.assertEqual(final['v14_source_sha256'], geometry['v14_file_sha256'])
        self.assertTrue(post['live_geometry_matches_disk'])
        self.assertTrue(post['old_scene_digests_preserved'])
        self.assertTrue(post['source_files_unchanged'])
        self.assertTrue(geometry['old_evaluated_disk_scene_digests_preserved'])
        self.assertTrue(geometry['duplicate_version_rejected'])
        self.assertEqual(len(geometry['clearance_checks']), 20)
        for support in geometry['clearance_checks']:
            self.assertGreaterEqual(support['vertical_clearance_m'], .02-3e-6)
        checked = [p for p in geometry['post_checks'] if 'not_checked' not in p]
        self.assertEqual(len(checked), 68)
        self.assertEqual(len(geometry['post_checks']) - len(checked), 48)
        self.assertEqual(final['lower_post_squares_not_checked'], 48)
        existing_negative = [p for p in checked if p['v13_min_vertical_gap_m'] < -3e-6]
        self.assertEqual(len(existing_negative), 16)
        self.assertEqual(final['lower_post_squares_existing_negative'], 16)
        for point in checked:
            if point['v13_min_vertical_gap_m'] >= -3e-6:
                self.assertGreaterEqual(point['v14_min_vertical_gap_m'], -3e-6)
        worsening = max(p['v13_min_vertical_gap_m'] - p['v14_min_vertical_gap_m']
                        for p in existing_negative)
        self.assertAlmostEqual(final['maximum_existing_negative_gap_worsening_m'], worsening)
        self.assertGreater(worsening, 0)
        self.assertEqual(visual['verdict'], 'revise')
        self.assertLess(visual['score'], 90)
        self.assertFalse(final['export_to_game'])
        self.assertEqual(final['portable_checkpoint'], '08')

    def test_l3_source_study_preserves_unresolved_component_mapping(self):
        path = ROOT / 'docs/refactoring/yellow-crane-l3-section-constraints.json'
        constraints = json.loads(path.read_text())
        evidence = HANDOFF / 'yellow-crane/evidence'
        report = json.loads((evidence / 'l3-section-constraint-study-01.json').read_text())
        prior = json.loads((evidence / 'post-contact-v14-runtime.json').read_text())
        self.assertEqual(report['constraints_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(report['source_file_sha256'], prior['source_blend_sha256']['14'])
        self.assertEqual(report['source_datums']['stations_m'], [26.,25.4,24.56,23.5,23.,22.2])
        for key in ('column_top_verified','transfer_to_exterior',
                    'inner_ring_surface_verified','plan_correspondence_verified'):
            self.assertFalse(constraints[key])
            self.assertFalse(report['source_datums'][key])
        self.assertTrue(report['actual_blender_readback'])
        self.assertFalse(report['exterior_geometry_modified'])
        self.assertFalse(report['export_to_game'])
        self.assertEqual(report['before'], report['after'])
        disk = report['saved_file_readback']
        self.assertTrue(disk['verified'])
        self.assertTrue(disk['old_scene_geometry_matches'])
        self.assertTrue(disk['runtime_section_counts_match'])
        self.assertTrue(disk['duplicate_version_rejected_without_scene_switch'])
        self.assertEqual(disk['scene_count'], report['prior_scene_count']+1)
        self.assertEqual(report['mesh_metadata']['Third_canopy']['section_segments'], 52)
        self.assertEqual(report['mesh_metadata']['L3_posts']['section_segments'], 10)
        self.assertAlmostEqual(report['support_station_minus_estimated_post_top_m'],
                               24.56-report['estimated_post_top_m'])
        self.assertGreater(report['support_station_minus_estimated_post_top_m'], 0.)
        self.assertEqual(report['exterior_checkpoint'], '14')
        self.assertEqual(report['portable_checkpoint'], '08')

    def test_post_preflight_is_not_promoted_to_runtime_acceptance(self):
        report = json.loads((HANDOFF / 'yellow-crane/evidence/post-contact-v14-source-preflight.json').read_text())
        self.assertFalse(report['actual_blender_readback'])
        self.assertFalse(report['solid_intersection_verified'])
        self.assertFalse(report['geometry_modified'])
        self.assertFalse(report['export_to_game'])
        self.assertEqual(report['summary']['posts'], 116)
        self.assertEqual(len(report['rows']), 116)
        for status,count in [('full',68),('partial',8),('none',40)]:
            self.assertEqual(sum(r['v14']['coverage'] == status for r in report['rows']), count)
        negative = [r for r in report['rows'] if r['v14']['coverage'] == 'full'
                    and r['v14']['vertical_gap_m'] < -3e-6]
        self.assertEqual(len(negative), 16)
        self.assertTrue(all(r['floor'] == 'L3' for r in negative))
        self.assertTrue(all(r['footprint_sides'] == 16 for r in report['rows']))
        self.assertIn('NOT actual Blender', report['limits'][0])

    def test_stored_mesh_witnesses_do_not_claim_runtime_acceptance(self):
        evidence = HANDOFF / 'yellow-crane/evidence'
        report = json.loads((evidence / 'post-contact-v14-stored-mesh.json').read_text())
        prior = json.loads((evidence / 'exterior-whitebox-14-readback.json').read_text())
        self.assertEqual(report['source_blend_sha256'], prior['v14_file_sha256'])
        self.assertTrue(report['stored_mesh_readback'])
        self.assertTrue(report['source_blend_matches_prior_runtime_evidence'])
        for key in ('actual_blender_readback', 'runtime_loop_triangles_verified',
                    'solid_intersection_verified', 'geometry_modified', 'export_to_game'):
            self.assertFalse(report[key], key)
        self.assertEqual(len(report['rows']), 116)
        self.assertEqual(len(report['mesh_metadata']), 8)
        for crosscheck in report['extraction_fixture_crosscheck'].values():
            self.assertEqual(crosscheck['maximum_float32_generator_coordinate_difference_m'], 0.)
            self.assertTrue(crosscheck['polygon_indices_match_generator'])
        for version in ('v13', 'v14'):
            negative = [r for r in report['rows'] if r[version]['negative_on_both']]
            self.assertEqual(len(negative), 16)
            self.assertTrue(all(r['floor'] == 'L3' for r in negative))
            self.assertEqual(report['summary'][version]['negative_on_both'], 16)
            self.assertEqual(report['summary'][version]['witnesses_inside_both_retriangulations'], 16)
            for diagonal in (0, 1):
                coverage = {state: sum(r[version]['diagonals'][diagonal]['coverage'] == state
                                       for r in report['rows']) for state in ('full', 'partial', 'none')}
                self.assertEqual(coverage, {'full': 68, 'partial': 8, 'none': 40})
                self.assertEqual(coverage, report['summary'][version]['coverage_by_diagonal'][diagonal])
            for row in negative:
                witness = row[version]['interior_witness']
                self.assertIsNotNone(witness)
                self.assertEqual(row['footprint_sides'], 16)
                self.assertLess(witness['point_m'][2], row['top_m'])
                lo, hi = witness['common_vertical_interval_m']
                self.assertLess(lo, witness['point_m'][2])
                self.assertLess(witness['point_m'][2], hi)
                self.assertGreater(witness['interval_half_width_m'], 0.)
                self.assertAlmostEqual(witness['post_winding_number'], 1.)
                self.assertEqual(len(witness['roof_winding_numbers']), 2)
                for winding in witness['roof_winding_numbers']:
                    self.assertAlmostEqual(winding, 1.)
        for diagonal in (0, 1):
            worsening = max(r['v13']['diagonals'][diagonal]['vertical_gap_m'] -
                            r['v14']['diagonals'][diagonal]['vertical_gap_m']
                            for r in report['rows'] if r['v13']['negative_on_both'])
            self.assertAlmostEqual(worsening, report['summary'][
                'maximum_existing_negative_worsening_by_diagonal_m'][diagonal])
        self.assertIn('NOT actual Blender', report['limits'][0])

    def test_runtime_contact_evidence_is_bounded_and_matches_stored_meshes(self):
        evidence=HANDOFF / 'yellow-crane/evidence'
        report=json.loads((evidence / 'post-contact-v14-runtime.json').read_text())
        stored_path=evidence / 'post-contact-v14-stored-mesh.json'
        stored=json.loads(stored_path.read_text())
        self.assertTrue(report['actual_blender_readback'])
        self.assertTrue(report['runtime_loop_triangles_verified'])
        self.assertTrue(report['bounded_runtime_interior_witnesses_verified'])
        self.assertFalse(report['solid_intersection_verified'])
        self.assertFalse(report['geometry_modified'])
        self.assertFalse(report['export_to_game'])
        self.assertEqual(report['geometry_before'],report['geometry_after'])
        self.assertEqual(report['source_blend_sha256']['14'],stored['source_blend_sha256'])
        self.assertEqual(report['stored_report_sha256'],hashlib.sha256(stored_path.read_bytes()).hexdigest())
        self.assertEqual(len(report['mesh_metadata']),8)
        for name,mesh in report['mesh_metadata'].items():
            self.assertEqual(mesh['coordinates_faces_sha256'],stored['mesh_metadata'][name]['coordinates_faces_sha256'])
            self.assertEqual(len(mesh['loop_triangles_sha256']),64)
            self.assertGreater(mesh['loop_triangles'],0)
        for name,digest in report['helper_sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / 'tools/blender' / name).read_bytes()).hexdigest(),digest)
        self.assertEqual(len(report['rows']),116)
        for version in ('v13','v14'):
            summary=report['summary'][version]
            self.assertEqual(summary['coverage'],{'full':68,'partial':8,'none':40})
            self.assertEqual(summary['negative_full_coverage'],16)
            self.assertEqual(summary['runtime_interior_witnesses'],16)
            negatives=[r for r in report['rows'] if r[version]['negative_full_coverage']]
            self.assertEqual(len(negatives),16)
            self.assertTrue(all(r['floor']=='L3' for r in negatives))
            for state,count in summary['coverage'].items():
                self.assertEqual(sum(r[version]['probe']['coverage']==state for r in report['rows']),count)
            gaps=[]
            for row in report['rows']:
                entry=row[version]
                if entry['probe']['coverage']=='full': gaps.append(entry['vertical_gap_m'])
                witness=entry['interior_witness']
                if entry['negative_full_coverage']:
                    self.assertEqual(entry['probe']['coverage'],'full')
                    self.assertLess(entry['vertical_gap_m'],-3e-6)
                    self.assertIsNotNone(witness)
                    self.assertAlmostEqual(witness['actual_post_winding_number'],1.)
                    self.assertEqual(len(witness['roof_winding_numbers']),1)
                    self.assertAlmostEqual(witness['roof_winding_numbers'][0],1.)
                    lo,hi=witness['common_vertical_interval_m']
                    self.assertLess(lo,witness['point_m'][2])
                    self.assertLess(witness['point_m'][2],hi)
                    self.assertLess(witness['point_m'][2],row['top_m'])
                else: self.assertIsNone(witness)
            self.assertAlmostEqual(min(gaps),summary['minimum_full_coverage_gap_m'])
        worsening=max(r['v13']['vertical_gap_m']-r['v14']['vertical_gap_m']
                      for r in report['rows'] if r['v13']['negative_full_coverage'])
        self.assertAlmostEqual(worsening,report['summary']['maximum_existing_negative_worsening_m'])

    def test_snapshot_roundtrip_evidence(self):
        proof = json.loads((HANDOFF / 'yellow-crane/portability-verification.json').read_text(encoding='utf-8'))
        artifact = ROOT / proof['artifact']
        self.assertEqual(hashlib.sha256(artifact.read_bytes()).hexdigest(), proof['artifact_sha256'])
        self.assertEqual(proof['mesh_count'], 33)
        self.assertEqual(proof['triangles'], 106008)
        self.assertEqual(proof['source_geometry_sha256'], proof['roundtrip_geometry_sha256'])
        self.assertEqual(proof['contents']['scenes'], ['HHL_Exterior_Whitebox_08_Portable'])
        for key in ('images', 'texts', 'libraries', 'sounds', 'movieclips', 'fonts'):
            self.assertEqual(proof['contents'][key], [], key)
        self.assertEqual(proof['external_dependencies'], [])
        self.assertEqual(proof['drivers'], [])
        self.assertTrue(proof['source_file_unchanged'])
        self.assertTrue(proof['existing_scene_geometry_preserved'])

    def test_omx_allowlist_excludes_private_and_runtime_files(self):
        ignored = [
            '.omx/state/notepad-before-context-portability.md',
            '.omx/state/yellow-crane-replica/ralph-progress.json',
            '.omx/references/yellow-crane/figure-2-2-1.jpeg',
            '.omx/references/yellow-crane/exterior-whitebox-08.blend',
            '.omx/experiments/build_yellow_crane_unverified.py',
            '.omx/project-memory.json', '.omx/future-runtime/cache.json',
            '.omx/plans/.DS_Store', '.omx/plans/secret.json',
        ]
        included = ['.omx/notepad.md', '.omx/plans/context-portability-20260908.md']
        for name in ignored + included:
            result = subprocess.run(
                ['git', 'check-ignore', '--no-index', '-q', name],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0 if name in ignored else 1,
                             f'{name}: {result.stderr}')

    def test_source_inventory_does_not_bundle_images(self):
        inventory = json.loads((HANDOFF / 'yellow-crane/source-inventory.json').read_text(encoding='utf-8'))
        self.assertFalse(inventory['images_distributed'])
        self.assertEqual(len(inventory['originals']), 10)
        for original in inventory['originals']:
            self.assertEqual(len(original['sha256']), 64)
            self.assertGreater(original['bytes_expected'], 0)
        for path in HANDOFF.rglob('*'):
            self.assertNotIn(path.suffix.lower(), {'.jpg', '.jpeg', '.png', '.blend1'})


if __name__ == '__main__':
    unittest.main()
