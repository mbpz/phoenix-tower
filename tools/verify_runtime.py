#!/usr/bin/env python3
"""Bounded native smoke/render-stress verification (stdlib only).

Build separately, then run, for example:
  python3 tools/verify_runtime.py --load saves/verify_plaque.ptw --screenshot
  python3 tools/verify_runtime.py --riverside --measure-fps --duration 30
  python3 tools/verify_runtime.py --stress 10000 --warmup 10 --duration 30 --timeout 120

Duration is the observation window AFTER requested startup evidence and warmup;
timeout bounds observation; cleanup adds a 5-second termination grace, forced
reaping if needed, and byte-bounded final parsing (not a hard wall-clock limit).
Stress runs and --measure-fps require at least three FPS samples in that window.
This checks evidence, not a portable performance target. Only --stress creates
render-stress entities; --riverside keeps its real editable blocks.
Parsing fails closed above 64 KiB per line or 64 MiB total; these are not disk
quotas. Repeated SIGINT is ignored during owned-child cleanup in the CLI.
Raw stdout AND stderr are retained in runtime.log, including known ICU diagnostics.
No save keys are sent, save slots are never written, and screenshots are opt-in.
The binary must be built from --root: the game embeds its asset/save root at build
time. Do not interact with the window or run another capture during verification.
"""
import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import statistics
import subprocess
import tempfile
import time
import threading


ROOT = Path(__file__).resolve().parents[1]
KNOWN_ICU = 'ICU4X data error: No segmentation model for complex script: Chinese/Japanese'
ANSI = re.compile(r'\x1b\[[0-?]*[ -/]*[@-~]')
FPS = re.compile(r'基准采样：FPS\s+([0-9]+(?:\.[0-9]+)?)\b')
SPAWN = re.compile(r'压力测试：生成\s+(\d+)\s+块积木')
UI = re.compile(r'lunex root dimension\s*=.*Vec2\(([0-9.]+),\s*([0-9.]+)\)')
IMPORT = re.compile(r'PHOENIX_LOAD 导入完成：(.*?)（\d+ 个积木）')
SHOT = re.compile(r'离屏截图命令已发送（saves/(screenshot_\d+\.png)）')
TIMESTAMP = re.compile(r'^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2}))\s+')
ERROR = re.compile(
    r'^(?:ERROR\b|FATAL\b|ICU4X data error:|wgpu error:|Validation Error|'
    r'LLVM ERROR:|panic(?:ked)?\b|thread .+ panicked\b)', re.IGNORECASE)
MIN_FPS_SAMPLES = 3
MAX_LOG_LINE_BYTES = 64 * 1024
MAX_LOG_PARSE_BYTES = 64 * 1024 * 1024


class LogEvidence:
    """Bounded diagnostic excerpts plus timestamped samples; full log stays on disk."""

    def __init__(self, launch_wall=None):
        self.launch_wall = launch_wall
        self.untimed_fps_count = 0
        self.samples = []
        self.spawn_counts = []
        self.imports = set()
        self.shots = set()
        self.ui_ready = False
        self.riverside_ready = False
        self.known_icu_count = 0
        self.incomplete_known_icu_tail_count = 0
        self.error_count = 0
        self.error_examples = []

    def feed(self, line, elapsed, *, terminated_tail=False):
        line = ANSI.sub('', line).strip()
        timestamp = TIMESTAMP.match(line)
        message = line[timestamp.end():].lstrip() if timestamp else line
        if line == KNOWN_ICU:
            self.known_icu_count += 1
        elif (terminated_tail and self.known_icu_count
              and line.startswith('ICU4X data error: No segmentation model for complex script:')
              and KNOWN_ICU.startswith(line)):
            # Only an unterminated final fragment after OUR normal termination,
            # and only a prefix of the already-observed exact known diagnostic.
            # Keep it separate from complete diagnostics; raw bytes are retained.
            self.incomplete_known_icu_tail_count += 1
        elif ERROR.search(message):
            self.error_count += 1
            if len(self.error_examples) < 20:
                self.error_examples.append(line)
        if 'RIVERSIDE_READY blocks=8 editable=true' in message:
            self.riverside_ready = True
        if match := FPS.search(line):
            fps = float(match[1])
            sample_time = elapsed
            if self.launch_wall is not None:
                # Membership follows producer time, not delayed file-reader time.
                # Game tracing emits UTC timestamps; untimed samples cannot prove
                # that they were generated after warmup and are never accepted.
                sample_time = None
                if timestamp:
                    try:
                        emitted = datetime.fromisoformat(timestamp[1].replace('Z', '+00:00'))
                        sample_time = emitted.timestamp() - self.launch_wall
                    except ValueError:
                        pass
                if sample_time is None:
                    self.untimed_fps_count += 1
            if math.isfinite(fps) and sample_time is not None:
                self.samples.append({'seconds_after_launch': sample_time, 'fps': fps})
        if match := SPAWN.search(line):
            self.spawn_counts.append(int(match[1]))
        if match := UI.search(line):
            try:
                self.ui_ready |= all(math.isfinite(float(v)) and float(v) > 0
                                     for v in match.groups())
            except ValueError:
                pass
        if match := IMPORT.search(line):
            self.imports.add(match[1])
        if match := SHOT.search(line):
            self.shots.add(match[1])

    def ready(self, *, stress, load):
        return (self.ui_ready and (not stress or self.spawn_counts == [stress])
                and (load is None or str(load) in self.imports))

    def fps_summary(self, start, end):
        samples = [sample for sample in self.samples
                   if start <= sample['seconds_after_launch'] <= end]
        values = [sample['fps'] for sample in samples]
        return {'count': len(values), 'median': statistics.median(values) if values else None,
                'min': min(values) if values else None, 'samples': samples}


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--root', type=Path, default=ROOT, help='root used to build the binary')
    parser.add_argument('--binary', type=Path, help='default: ROOT/target/debug/phoenix-tower[.exe]')
    parser.add_argument('--output-dir', type=Path,
                        help='parent of unique run directories (default: ROOT/saves/verification)')
    parser.add_argument('--duration', type=float, default=15, help='post-warmup observation seconds')
    parser.add_argument('--timeout', type=float, default=90, help='absolute lifetime limit in seconds')
    parser.add_argument('--warmup', type=float, default=5, help='seconds after startup evidence to ignore')
    parser.add_argument('--measure-fps', action='store_true',
                        help='require continuous FPS evidence for smoke/riverside (stress always requires it)')
    parser.add_argument('--riverside', action='store_true', help='verify editable Blender diorama')
    parser.add_argument('--stress', type=int, default=0, help='render-stress block count, 0 for smoke')
    parser.add_argument('--load', type=Path, help='read-only .ptw fixture; relative to root')
    parser.add_argument('--screenshot', action='store_true', help='request fresh screenshot (smoke only)')
    args = parser.parse_args(argv)
    for name in ('duration', 'timeout', 'warmup'):
        value = getattr(args, name)
        if not math.isfinite(value) or value < 0 or (name != 'warmup' and value == 0):
            parser.error(f'--{name} must be finite and {"nonnegative" if name == "warmup" else "positive"}')
    if args.timeout <= args.duration + args.warmup:
        parser.error('--timeout must exceed --duration + --warmup, allowing time for startup')
    if args.stress < 0:
        parser.error('--stress must be nonnegative')
    if args.stress and (args.load or args.screenshot):
        parser.error('stress runs cannot load (imports remove stress entities) or capture screenshots')
    if args.riverside and (args.load or args.stress):
        parser.error('riverside runs cannot also load or stress')
    args.root = args.root.expanduser().resolve()
    args.binary = (args.binary.expanduser().resolve() if args.binary else
                   args.root / 'target' / 'debug' / ('phoenix-tower.exe' if os.name == 'nt' else 'phoenix-tower'))
    args.output_dir = (args.output_dir.expanduser().resolve() if args.output_dir else
                       args.root / 'saves' / 'verification')
    if args.load:
        args.load = args.load.expanduser()
        args.load = (args.root / args.load).resolve()
    return args


def build_environment(args):
    env = {key: value for key, value in os.environ.items()
           if not key.upper().startswith('PHOENIX_')}
    env.update(RUST_LOG='info', RUST_BACKTRACE='1', NO_COLOR='1', PHOENIX_UI_PROBE='1')
    if args.stress:
        env['PHOENIX_STRESS'] = str(args.stress)
    if args.measure_fps and not args.stress:
        env['PHOENIX_PERF_PROBE'] = '1'
    if args.load:
        env['PHOENIX_LOAD'] = str(args.load)
    if args.screenshot:
        env['PHOENIX_SHOT'] = '1'
    return env


def fresh_screenshot(saves, before, launch_wall, names):
    # Require our command log, a previously nonexistent path, a fresh timestamp,
    # and PNG end marker: existence alone can observe an unfinished async write.
    for name in sorted(names):
        path = saves / name
        try:
            if path in before or path.stat().st_mtime < launch_wall:
                continue
            with path.open('rb') as image:
                if image.read(8) != b'\x89PNG\r\n\x1a\n':
                    continue
                image.seek(-12, os.SEEK_END)
                if image.read() == b'\x00\x00\x00\x00IEND\xaeB`\x82':
                    return path
        except (OSError, ValueError):
            continue
    return None


def stop_owned_process(process):
    """Never search/kill by executable name or affect another game's process."""
    if process is not None and process.poll() is None:
        try:
            process.terminate()
        except ProcessLookupError:
            pass  # The child can exit between poll() and terminate().
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def run_verification(args, *, command=None):
    """Run one owned child; command override is for headless lifecycle tests only."""
    args.output_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    run_dir = Path(tempfile.mkdtemp(prefix=f'{stamp}-', dir=args.output_dir))
    log_path = run_dir / 'runtime.log'
    summary_path = run_dir / 'summary.json'
    saves = args.root / 'saves'
    shots_before = set(saves.glob('screenshot_*.png')) if args.screenshot else set()
    launch_wall = time.time()
    start = time.monotonic()
    evidence = LogEvidence(launch_wall=launch_wall)
    process = None
    ready_at = None
    screenshot = None
    stop_reason = 'launch_error'
    exit_code = None
    failures = []
    pending = b''
    parsed_bytes = 0
    log_limit = None

    def drain(reader, final=False, terminated_tail=False):
        nonlocal pending, parsed_bytes, log_limit
        if log_limit:
            return False
        # Bound both per-poll work and the final drain. One extra byte detects
        # overflow; the raw file remains intact, and incomplete analysis fails.
        chunk = reader.read(min(1024 * 1024, MAX_LOG_PARSE_BYTES - parsed_bytes + 1))
        parsed_bytes += len(chunk)
        if parsed_bytes > MAX_LOG_PARSE_BYTES:
            log_limit = f'Log parse limit exceeded ({MAX_LOG_PARSE_BYTES} bytes).'
            pending = b''
            return False
        lines = (pending + chunk).split(b'\n')
        pending = lines.pop()
        if len(pending) > MAX_LOG_LINE_BYTES or any(len(line) > MAX_LOG_LINE_BYTES for line in lines):
            log_limit = f'Log line limit exceeded ({MAX_LOG_LINE_BYTES} bytes).'
            pending = b''
            return False
        elapsed = time.monotonic() - start
        for line in lines:
            evidence.feed(line.decode('utf-8', errors='replace'), elapsed)
        if final and pending:
            evidence.feed(pending.decode('utf-8', errors='replace'), elapsed,
                          terminated_tail=terminated_tail)
            pending = b''
        return bool(chunk)

    with log_path.open('wb') as output, log_path.open('rb') as reader:
        try:
            if args.load and not args.load.is_file():
                raise FileNotFoundError(f'Import fixture does not exist: {args.load}')
            process = subprocess.Popen(command or [str(args.binary), *(['--riverside'] if args.riverside else [])], cwd=args.root,
                                       env=build_environment(args), stdout=output,
                                       stderr=subprocess.STDOUT)
            while True:
                drain(reader)
                elapsed = time.monotonic() - start
                exit_code = process.poll()
                if exit_code is not None:
                    stop_reason = 'early_exit'
                    break
                if log_limit:
                    stop_reason = 'log_limit'
                    break
                if evidence.error_count:
                    stop_reason = 'runtime_error'
                    break
                if elapsed >= args.timeout:
                    stop_reason = 'timeout'
                    break
                if args.screenshot:
                    screenshot = fresh_screenshot(saves, shots_before, launch_wall, evidence.shots)
                if ready_at is None and evidence.ready(stress=args.stress, load=args.load) and (not args.riverside or evidence.riverside_ready):
                    if not args.screenshot or screenshot:
                        ready_at = elapsed
                if ready_at is not None and elapsed >= ready_at + args.warmup + args.duration:
                    stop_reason = 'duration_complete'
                    break
                time.sleep(min(0.025, max(0, args.timeout - elapsed)))
        except KeyboardInterrupt:
            stop_reason = 'interrupted'
            failures.append('Verifier interrupted; owned child was stopped.')
        except OSError as error:
            failures.append(f'{type(error).__name__}: {error}')
            stop_reason = 'launch_error' if process is None else 'verifier_error'
        finally:
            # Ctrl-C has already selected failure above. Further SIGINT must
            # not strand the owned child during its bounded termination grace.
            # Signal handlers may only be installed by Python's main thread.
            main_thread = threading.current_thread() is threading.main_thread()
            previous_sigint = signal.signal(signal.SIGINT, signal.SIG_IGN) if main_thread else None
            try:
                if process is not None and stop_reason == 'duration_complete':
                    exit_code = process.poll()
                    if exit_code is not None:
                        stop_reason = 'early_exit'
                stop_owned_process(process)
                # Child is reaped before bounded final parsing of late errors.
                while drain(reader):
                    pass
                drain(reader, final=True, terminated_tail=stop_reason == 'duration_complete')
            finally:
                if main_thread:
                    signal.signal(signal.SIGINT, previous_sigint)
    if log_limit:
        failures.append(log_limit + ' Evidence is incomplete; raw output remains in runtime.log.')
    elapsed = time.monotonic() - start
    window_start = ready_at + args.warmup if ready_at is not None else elapsed
    window_end = min(window_start + args.duration, elapsed)
    fps = evidence.fps_summary(window_start, window_end)
    if stop_reason != 'duration_complete':
        failures.append(f'Run did not complete its observation window: {stop_reason}.')
    if evidence.error_count:
        failures.append(f'{evidence.error_count} unexpected error/panic diagnostic(s); see runtime.log.')
    if not evidence.ui_ready:
        failures.append('Missing positive UI layout evidence.')
    if args.load and str(args.load) not in evidence.imports:
        failures.append('Missing successful import evidence for the requested fixture.')
    if args.stress and evidence.spawn_counts != [args.stress]:
        failures.append(f'Missing or mismatched stress spawn log: expected [{args.stress}].')
    fps_required = bool(args.stress or args.measure_fps)
    if fps_required and evidence.untimed_fps_count:
        failures.append(f'{evidence.untimed_fps_count} FPS sample(s) missing valid producer timestamps.')
    if fps_required and fps['count'] < MIN_FPS_SAMPLES:
        failures.append(f'Need at least {MIN_FPS_SAMPLES} post-warmup FPS samples; got {fps["count"]}.')
    if args.riverside and not evidence.riverside_ready:
        failures.append('Missing editable riverside startup evidence.')
    screenshot_path = None
    if args.screenshot:
        screenshot = fresh_screenshot(saves, shots_before, launch_wall, evidence.shots)
        if screenshot is None:
            failures.append('Missing fresh completed screenshot from this run.')
        else:
            screenshot_path = run_dir / screenshot.name
            try:
                # Exclusive create preserves the no-overwrite contract even for artifacts.
                with screenshot.open('rb') as source, screenshot_path.open('xb') as target:
                    shutil.copyfileobj(source, target)
            except OSError as error:
                failures.append(f'Screenshot copy failed: {error}')
                screenshot_path = None
    summary = {
        'schema_version': 1,
        'status': 'failed' if failures else 'passed', 'stop_reason': stop_reason,
        'failures': failures, 'binary': str(args.binary), 'root': str(args.root),
        'mode': 'riverside' if args.riverside else ('render_stress' if args.stress else 'smoke'), 'requested_stress': args.stress,
        'stress_spawn_counts': evidence.spawn_counts, 'load_fixture': str(args.load) if args.load else None,
        'ui_ready': evidence.ui_ready, 'ready_seconds_after_launch': ready_at,
        'warmup_seconds': args.warmup, 'duration_seconds': args.duration,
        'timeout_seconds': args.timeout, 'elapsed_seconds': elapsed,
        'sample_window_seconds_after_launch': [window_start, window_end],
        'fps_required': fps_required,
        'fps': fps, 'fps_timing': 'producer_utc_relative_to_launch',
        'untimed_fps_count': evidence.untimed_fps_count,
        'exit_code_before_cleanup': exit_code,
        'exit_code': process.returncode if process else None,
        'owned_pid': process.pid if process else None,
        'owned_child_reaped': process is None or process.poll() is not None,
        'diagnostics': {'known_icu_segmentation_count': evidence.known_icu_count,
                        'incomplete_known_icu_tail_count': evidence.incomplete_known_icu_tail_count,
                        'error_count': evidence.error_count, 'error_examples': evidence.error_examples},
        'log_limits': {'line_bytes': MAX_LOG_LINE_BYTES, 'parse_bytes': MAX_LOG_PARSE_BYTES,
                       'read_bytes': parsed_bytes, 'exceeded': log_limit,
                       'raw_bytes': log_path.stat().st_size},
        'log_path': str(log_path), 'summary_path': str(summary_path),
        'screenshot_path': str(screenshot_path) if screenshot_path else None,
    }
    summary_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return summary


def main():
    result = run_verification(parse_args())
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
