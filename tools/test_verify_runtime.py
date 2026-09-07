"""Headless stdlib tests: python3 -B -m unittest discover -s tools -v."""
import json
import os
import signal
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

from verify_runtime import LogEvidence, build_environment, parse_args, run_verification


UI = 'INFO lunex root dimension = Dimension(Vec2(1280.0, 720.0))'
ICU = 'ICU4X data error: No segmentation model for complex script: Chinese/Japanese'


class LogTests(unittest.TestCase):
    def test_fps_filters_warmup_and_reports_distribution_not_single_peak(self):
        evidence = LogEvidence()
        evidence.feed('INFO 🧪 基准采样：FPS 1.0', 1)
        evidence.feed('INFO 🧪 基准采样：FPS 60.0', 6)
        evidence.feed('INFO 🧪 基准采样：FPS 40.0', 7)
        evidence.feed('INFO 🧪 基准采样：FPS 50.0', 8)
        evidence.feed('INFO 🧪 基准采样：FPS 500.0', 20)
        evidence.feed('INFO fps with lunex panel: 999', 8)
        self.assertEqual(evidence.fps_summary(5, 10), {
            'count': 3, 'median': 50.0, 'min': 40.0,
            'samples': [{'seconds_after_launch': 6, 'fps': 60.0},
                        {'seconds_after_launch': 7, 'fps': 40.0},
                        {'seconds_after_launch': 8, 'fps': 50.0}],
        })
        self.assertIsNone(evidence.fps_summary(30, 40)['median'])

    def test_only_exact_known_icu_diagnostic_is_nonfatal(self):
        evidence = LogEvidence()
        evidence.feed(ICU, 0)
        evidence.feed('ICU4X data error: something else', 0)
        evidence.feed(ICU + ' ERROR renderer failed', 0)
        evidence.feed("thread 'main' panicked at src/main.rs:42", 0)
        evidence.feed('\x1b[31mERROR\x1b[0m GPU lost', 0)
        self.assertEqual(evidence.known_icu_count, 1)
        self.assertEqual(evidence.error_count, 4)

    def test_runtime_fps_requires_producer_timestamp(self):
        evidence = LogEvidence(launch_wall=0)
        evidence.feed('INFO 基准采样：FPS 999.0', 9)
        evidence.feed('1970-01-01T00:00:01.250000Z INFO 基准采样：FPS 60.0', 9)
        self.assertEqual(evidence.untimed_fps_count, 1)
        self.assertEqual(evidence.fps_summary(8, 10)['count'], 0)
        self.assertEqual(evidence.fps_summary(1, 2)['samples'],
                         [{'seconds_after_launch': 1.25, 'fps': 60.0}])

    def test_readiness_requires_valid_ui_and_exact_requested_stress_count(self):
        evidence = LogEvidence()
        evidence.feed('INFO lunex root dimension = Dimension(Vec2(0.0, 720.0))', 0)
        evidence.feed('INFO 压力测试：生成 100 块积木', 0)
        self.assertFalse(evidence.ready(stress=100, load=None))
        evidence.feed(UI, 1)
        self.assertTrue(evidence.ready(stress=100, load=None))
        self.assertFalse(evidence.ready(stress=1000, load=None))
        fixture = Path('/tmp/fixture.ptw')
        self.assertFalse(evidence.ready(stress=0, load=fixture))
        evidence.feed('INFO PHOENIX_LOAD 导入完成：wrong.ptw（1 个积木）', 2)
        self.assertFalse(evidence.ready(stress=0, load=fixture))
        evidence.feed(f'INFO PHOENIX_LOAD 导入完成：{fixture}（1 个积木）', 3)
        self.assertTrue(evidence.ready(stress=0, load=fixture))

    def test_import_paths_do_not_masquerade_as_log_severity(self):
        evidence = LogEvidence()
        for word in ('error', 'fatal', 'panicked'):
            evidence.feed(f'2026-09-05T17:00:00Z  INFO save: PHOENIX_LOAD 导入完成：/tmp/{word}.ptw（1 个积木）', 0)
        self.assertEqual(evidence.error_count, 0)
        evidence.feed('2026-09-05T17:00:00Z ERROR renderer: validation failed', 0)
        self.assertEqual(evidence.error_count, 1)

    def test_isolates_phoenix_flags_and_forces_visible_logging(self):
        args = parse_args([])
        with patch.dict(os.environ, {'PHOENIX_LOAD': '/user/slot.ptw',
                                     'PHOENIX_STRESS': '50000', 'PHOENIX_SHOT': '1',
                                     'PHOENIX_TEXT3D_PROBE': '1', 'RUST_LOG': 'off'}):
            env = build_environment(args)
        self.assertEqual({k: v for k, v in env.items() if k.startswith('PHOENIX_')},
                         {'PHOENIX_UI_PROBE': '1'})
        self.assertEqual(env['RUST_LOG'], 'info')

    def test_rejects_incompatible_and_unbounded_cli_settings(self):
        import contextlib
        import io
        for flags in (['--stress', '100', '--screenshot'],
                      ['--stress', '100', '--load', 'fixture.ptw'],
                      ['--duration', 'nan'], ['--timeout', 'inf'],
                      ['--duration', '-1'], ['--stress', '-1'],
                      ['--warmup', '-1'], ['--timeout', '1']):
            with self.subTest(flags=flags), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    parse_args(flags)


class ProcessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        (self.root / 'saves').mkdir()
        self.slot = self.root / 'saves' / 'slot1.ptw'
        self.slot.write_bytes(b'user data must stay untouched')

    def config(self, *flags):
        return parse_args(['--root', str(self.root), '--output-dir', str(self.root / 'runs'),
                           '--binary', sys.executable, '--duration', '0.15',
                           '--warmup', '0', '--timeout', '5', *flags])

    def run_fake(self, code, *flags):
        processes = []
        real_popen = subprocess.Popen

        def track(*args, **kwargs):
            process = real_popen(*args, **kwargs)
            processes.append(process)
            return process

        try:
            with patch('verify_runtime.subprocess.Popen', side_effect=track):
                result = run_verification(self.config(*flags),
                                          command=[sys.executable, '-u', '-X', 'utf8', '-c', code])
                self.assertTrue(all(p.poll() is not None for p in processes),
                                'verifier must reap its child before returning')
        finally:
            # A failing lifecycle regression must not leak its test child.
            for process in processes:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=5)
        self.assertEqual(len(processes), 1)
        self.assertIsNotNone(processes[0].poll(), 'owned child must always be reaped')
        self.assertEqual(self.slot.read_bytes(), b'user data must stay untouched')
        saved = json.loads(Path(result['summary_path']).read_text())
        self.assertEqual(saved['status'], result['status'])
        return result

    def test_success_stays_alive_and_uses_distinct_run_directories(self):
        code = f'import time; print({UI!r}, flush=True); time.sleep(10)'
        first = self.run_fake(code)
        second = self.run_fake(code)
        self.assertEqual(first['status'], 'passed', first)
        self.assertNotEqual(first['summary_path'], second['summary_path'])
        self.assertEqual(first['stop_reason'], 'duration_complete')

    def test_early_success_exit_is_still_failure_and_stderr_is_preserved(self):
        result = self.run_fake(f'import sys; print({UI!r}); print("stderr marker", file=sys.stderr)')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stop_reason'], 'early_exit')
        self.assertEqual(result['exit_code'], 0)
        self.assertIn('stderr marker', Path(result['log_path']).read_text())

    def test_missing_evidence_times_out_and_kills_only_our_child(self):
        start = time.monotonic()
        result = self.run_fake('import time; time.sleep(10)', '--timeout', '0.3')
        self.assertLess(time.monotonic() - start, 3)
        self.assertEqual(result['stop_reason'], 'timeout')
        self.assertEqual(result['status'], 'failed')
        self.assertTrue(any('UI' in error for error in result['failures']))

    def test_error_fails_immediately_even_after_ready(self):
        result = self.run_fake(f'import time; print({UI!r}); print("ERROR device lost"); time.sleep(10)')
        self.assertEqual(result['stop_reason'], 'runtime_error')
        self.assertEqual(result['diagnostics']['error_count'], 1)
        self.assertEqual(result['status'], 'failed')

    def test_known_icu_is_reported_without_hiding_raw_diagnostics(self):
        result = self.run_fake(f'import time; print({ICU!r}); print({UI!r}); time.sleep(10)')
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(result['diagnostics']['known_icu_segmentation_count'], 1)
        self.assertIn(ICU, Path(result['log_path']).read_text())

    def test_stress_requires_post_warmup_samples(self):
        code = (f'import time; from datetime import datetime, timezone; print({UI!r}); print("压力测试：生成 1000 块积木"); '
                'print(datetime.now(timezone.utc).isoformat() + " INFO 基准采样：FPS 100.0"); time.sleep(10)')
        result = self.run_fake(code, '--stress', '1000', '--warmup', '0.05')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['fps']['count'], 0)
        self.assertTrue(any('FPS' in error for error in result['failures']))

    def test_stress_success_with_multiple_live_samples(self):
        code = (f'import time; from datetime import datetime, timezone; print({UI!r}); print("压力测试：生成 1000 块积木"); '
                'time.sleep(0.05)\n'
                'for fps in [30, 40, 50, 60]:\n'
                ' print(datetime.now(timezone.utc).isoformat() + f" INFO 基准采样：FPS {fps}.0", flush=True); time.sleep(0.06)\n'
                'time.sleep(10)')
        result = self.run_fake(code, '--stress', '1000', '--duration', '0.6')
        self.assertEqual(result['status'], 'passed', result)
        self.assertEqual(result['fps']['count'], 4)
        self.assertEqual(result['fps']['median'], 45)
        self.assertEqual(result['stress_spawn_counts'], [1000])

    def test_backlogged_pre_warmup_samples_do_not_pass(self):
        # Deliberately old producer times make this independent of CI scheduling.
        code = (f'import sys, time; print({UI!r}); print("压力测试：生成 1000 块积木"); '
                'sys.stdout.write(("x" * 511 + "\\n") * 16384); '
                'print("2020-01-01T00:00:00Z INFO 基准采样：FPS 60.0\\n" * 3); '
                'time.sleep(10)')
        result = self.run_fake(code, '--stress', '1000', '--warmup', '0.1', '--duration', '0.6')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['fps']['count'], 0)

    def test_default_player_entry_and_historic_smoke_are_distinct(self):
        from verify_runtime import game_command
        self.assertEqual(game_command(parse_args([]))[-1], '--tower')
        args = parse_args(['--riverside'])
        self.assertEqual(game_command(args), [str(args.binary)])

    def test_riverside_fps_measurement_requires_post_warmup_samples(self):
        code = (f'import time; print({UI!r}); '
                'print("RIVERSIDE_READY blocks=8 editable=true"); time.sleep(10)')
        result = self.run_fake(code, '--riverside', '--measure-fps')
        self.assertEqual(result['status'], 'failed')
        self.assertTrue(result['fps_required'])
        self.assertTrue(any('FPS samples' in error for error in result['failures']))

    def test_riverside_fps_measurement_accepts_live_samples(self):
        code = (f'import time; from datetime import datetime, timezone; print({UI!r}); '
                'print("RIVERSIDE_READY blocks=8 editable=true"); time.sleep(0.05)\n'
                'for fps in [30, 40, 50, 60]:\n'
                ' print(datetime.now(timezone.utc).isoformat() + f" INFO 基准采样：FPS {fps}.0 source=ui", flush=True); time.sleep(0.06)\n'
                'time.sleep(10)')
        result = self.run_fake(code, '--riverside', '--measure-fps', '--duration', '0.6')
        self.assertEqual(result['status'], 'passed', result)
        self.assertEqual(result['fps']['count'], 4)
        self.assertEqual(result['fps']['median'], 45.0)
        self.assertEqual(result['mode'], 'riverside')

    def test_ui_measurement_rejects_untimed_samples_even_with_enough_timed_samples(self):
        code = (f'import time; from datetime import datetime, timezone; print({UI!r}); '
                'time.sleep(0.05)\n'
                'for fps in [30, 40, 50, 60]:\n'
                ' print(datetime.now(timezone.utc).isoformat() + f" INFO 基准采样：FPS {fps}.0", flush=True); time.sleep(0.06)\n'
                'print("INFO 基准采样：FPS 999.0"); time.sleep(10)')
        result = self.run_fake(code, '--measure-fps', '--duration', '0.6')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['untimed_fps_count'], 1)
        self.assertTrue(any('timestamps' in error for error in result['failures']))

    def test_stale_screenshot_never_satisfies_capture_request(self):
        (self.root / 'saves' / 'screenshot_1.png').write_bytes(b'old screenshot')
        result = self.run_fake(f'import time; print({UI!r}); time.sleep(10)',
                               '--screenshot', '--timeout', '0.3')
        self.assertEqual(result['status'], 'failed')
        self.assertIsNone(result['screenshot_path'])

    def test_fresh_complete_screenshot_is_copied_into_run_directory(self):
        # Minimal framing suffices for freshness/completion checks; not a GPU/image test.
        code = (f'import time; from pathlib import Path; print({UI!r}); '
                'Path("saves/screenshot_123.png").write_bytes('
                'b"\\x89PNG\\r\\n\\x1a\\n" + b"\\x00\\x00\\x00\\x00IEND\\xaeB`\\x82"); '
                'print("离屏截图命令已发送（saves/screenshot_123.png）"); time.sleep(10)')
        result = self.run_fake(code, '--screenshot')
        self.assertEqual(result['status'], 'passed', result)
        self.assertTrue(Path(result['screenshot_path']).is_file())

    def test_import_fixture_requires_evidence_and_remains_unchanged(self):
        fixture = self.root / 'fixture.ptw'
        fixture.write_bytes(b'fixture')
        code = (f'import time; print({UI!r}); '
                f'print({f"PHOENIX_LOAD 导入完成：{fixture}（1 个积木）"!r}); time.sleep(10)')
        result = self.run_fake(code, '--load', str(fixture))
        self.assertEqual(result['status'], 'passed', result)
        self.assertEqual(fixture.read_bytes(), b'fixture')

    def test_unterminated_stderr_error_is_not_lost_on_early_exit(self):
        result = self.run_fake('import sys; sys.stderr.write("ERROR partial line")')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['diagnostics']['error_count'], 1)
        self.assertIn('ERROR partial line', Path(result['log_path']).read_text())

    def test_own_termination_marks_only_exact_known_icu_tail_as_incomplete(self):
        tail = 'ICU4X data error: No segmentation model for complex script: '
        code = (f'import sys, time; print({ICU!r}); print({UI!r}); '
                f'sys.stderr.write({tail!r}); sys.stderr.flush(); time.sleep(10)')
        result = self.run_fake(code)
        self.assertEqual(result['status'], 'passed', result)
        self.assertEqual(result['diagnostics']['incomplete_known_icu_tail_count'], 1)
        self.assertTrue(Path(result['log_path']).read_text().endswith(tail))

    def test_complete_but_unknown_icu_line_remains_fatal(self):
        tail = 'ICU4X data error: No segmentation model for complex script: '
        result = self.run_fake(f'import time; print({ICU!r}); print({UI!r}); print({tail!r}); time.sleep(10)')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['diagnostics']['error_count'], 1)

    def test_interrupt_still_stops_and_reaps_owned_process(self):
        real_sleep = time.sleep
        interrupted = False

        def interrupt_once(seconds):
            nonlocal interrupted
            if not interrupted:
                interrupted = True
                raise KeyboardInterrupt
            real_sleep(seconds)

        with patch('verify_runtime.time.sleep', side_effect=interrupt_once):
            result = self.run_fake('import time; time.sleep(10)')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stop_reason'], 'interrupted')

    def test_unterminated_oversized_line_fails_without_losing_raw_bytes(self):
        payload = b'x' * 4097
        code = (f'import sys, time; print({UI!r}); '
                f'sys.stdout.buffer.write({payload!r}); sys.stdout.flush(); time.sleep(10)')
        with patch('verify_runtime.MAX_LOG_LINE_BYTES', 4096):
            result = self.run_fake(code)
        self.assertEqual(result['status'], 'failed', result)
        self.assertTrue(any('line limit' in item for item in result['failures']))
        self.assertIn(payload, Path(result['log_path']).read_bytes())

    def test_terminated_oversized_line_cannot_supply_ready_evidence(self):
        code = f'import time; print({UI!r} + "x" * 4096); time.sleep(10)'
        with patch('verify_runtime.MAX_LOG_LINE_BYTES', 4096):
            result = self.run_fake(code)
        self.assertEqual(result['status'], 'failed')
        self.assertFalse(result['ui_ready'])
        self.assertIn('line limit', result['log_limits']['exceeded'])

    def test_exact_line_and_total_budgets_preserve_utf8_evidence(self):
        line = ('中文' * 100).encode('utf-8')
        payload = UI.encode() + b'\n' + line + b'\n'
        code = f'import sys, time; sys.stdout.buffer.write({payload!r}); sys.stdout.flush(); time.sleep(10)'
        with patch('verify_runtime.MAX_LOG_LINE_BYTES', len(line)), \
                patch('verify_runtime.MAX_LOG_PARSE_BYTES', len(payload)):
            result = self.run_fake(code)
        self.assertEqual(result['status'], 'passed', result)
        self.assertIsNone(result['log_limits']['exceeded'])
        self.assertEqual(result['log_limits']['read_bytes'], len(payload))
        self.assertEqual(Path(result['log_path']).read_bytes(), payload)

    def test_limit_found_only_during_final_drain_still_fails(self):
        from verify_runtime import stop_owned_process

        def append_late_log(process):
            stop_owned_process(process)
            log = next((self.root / 'runs').glob('*/runtime.log'))
            with log.open('ab') as output:
                output.write(b'x' * 4097)

        with patch('verify_runtime.MAX_LOG_LINE_BYTES', 4096), \
                patch('verify_runtime.stop_owned_process', side_effect=append_late_log):
            result = self.run_fake(f'import time; print({UI!r}); time.sleep(10)')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stop_reason'], 'duration_complete')
        self.assertIn('line limit', result['log_limits']['exceeded'])

    def test_total_log_budget_fails_even_for_short_valid_lines(self):
        code = f'import time; print({UI!r}); print("safe line\\n" * 2000); time.sleep(10)'
        with patch('verify_runtime.MAX_LOG_PARSE_BYTES', 4096):
            result = self.run_fake(code)
        self.assertEqual(result['status'], 'failed', result)
        self.assertTrue(any('parse limit' in item for item in result['failures']))

    def test_repeated_sigint_during_cleanup_is_deferred_and_handler_restored(self):
        from verify_runtime import stop_owned_process
        previous = signal.getsignal(signal.SIGINT)

        def interrupt_cleanup(process):
            signal.raise_signal(signal.SIGINT)
            signal.raise_signal(signal.SIGINT)
            stop_owned_process(process)

        real_sleep = time.sleep
        interrupted = False

        def first_interrupt(seconds):
            nonlocal interrupted
            if not interrupted:
                interrupted = True
                signal.raise_signal(signal.SIGINT)
            real_sleep(seconds)

        with patch('verify_runtime.time.sleep', side_effect=first_interrupt), \
                patch('verify_runtime.stop_owned_process', side_effect=interrupt_cleanup):
            try:
                result = self.run_fake('import time; time.sleep(10)')
            except KeyboardInterrupt:
                self.fail('second SIGINT escaped owned-process cleanup')
        self.assertEqual(result['stop_reason'], 'interrupted')
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(signal.getsignal(signal.SIGINT), previous)

    def test_launch_failure_still_emits_machine_readable_report(self):
        config = self.config('--binary', str(self.root / 'missing-executable'))
        result = run_verification(config)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['stop_reason'], 'launch_error')
        self.assertTrue(Path(result['summary_path']).exists())


class RiversideTests(unittest.TestCase):
    def test_perf_probe_is_opt_in_and_does_not_duplicate_stress_sampler(self):
        self.assertFalse(parse_args([]).measure_fps)
        args = parse_args(['--riverside', '--measure-fps'])
        self.assertEqual(build_environment(args)['PHOENIX_PERF_PROBE'], '1')
        args = parse_args(['--stress', '1000', '--measure-fps'])
        self.assertNotIn('PHOENIX_PERF_PROBE', build_environment(args))

    def test_mode_is_explicit_and_exclusive(self):
        self.assertFalse(parse_args([]).riverside)
        self.assertTrue(parse_args(['--riverside', '--screenshot']).riverside)
        for extra in (['--stress', '1000'], ['--load', 'a.ptw']):
            with self.assertRaises(SystemExit):
                parse_args(['--riverside', *extra])

    def test_requires_exact_seed_log(self):
        evidence = LogEvidence()
        evidence.feed(UI, 0)
        evidence.feed('INFO RIVERSIDE_READY blocks=7 editable=true', 1)
        self.assertFalse(evidence.riverside_ready)
        evidence.feed('INFO RIVERSIDE_READY blocks=8 editable=true', 2)
        self.assertTrue(evidence.riverside_ready)


if __name__ == '__main__':
    unittest.main()
