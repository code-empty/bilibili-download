from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from importlib import metadata
from pathlib import Path
from typing import Any, Dict, Optional
from urllib.parse import parse_qs, urlparse
import shutil

from yt_dlp import YoutubeDL


def resolve_yt_dlp_version() -> str:
    try:
        return metadata.version("yt-dlp")
    except Exception:
        return "unknown"


YT_DLP_VERSION = resolve_yt_dlp_version()


def configure_stdio() -> None:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if callable(reconfigure):
            try:
                reconfigure(encoding="utf-8", errors="replace")
            except Exception:
                pass


def emit(event: str, payload: Dict[str, Any]) -> None:
    payload["event"] = event
    print(json.dumps(payload, ensure_ascii=False), flush=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="SnapDown worker")
    parser.add_argument("--task-json", required=True)
    return parser.parse_args()


def classify_error(message: str, url: str) -> str:
    msg = message.lower()
    if "winerror 10013" in msg:
        return "NETWORK_BLOCKED"
    if any(token in msg for token in ("login", "need login", "private", "sign in", "fresh cookies", "cookies are needed")):
        return "LOGIN_REQUIRED"
    if any(token in msg for token in ("forbidden", "region", "geo restriction", "not available in your country")):
        return "REGION_BLOCKED"
    if "drm" in msg:
        return "DRM_PROTECTED"
    if "video unavailable" in msg:
        return "UNSUPPORTED_URL"
    if "requested format" in msg:
        return "FORMAT_UNAVAILABLE"
    if "unsupported url" in msg:
        if any(host in url.lower() for host in ("bilibili.com", "b23.tv", "douyin.com", "iesdouyin.com", "youtube.com", "youtu.be")):
            return "NETWORK_FAIL"
        return "UNSUPPORTED_URL"
    return "NETWORK_FAIL"


def site_overrides(platform: str, options: Dict[str, Any]) -> Dict[str, Any]:
    if platform == "bilibili":
        options["noplaylist"] = True
    elif platform == "youtube":
        options["geo_bypass"] = True
    return options


def normalize_platform(url: str, hint: str = "") -> str:
    hint = (hint or "").strip().lower()
    if hint in {"bilibili", "douyin", "youtube", "b23", "yt", "youtube.com"}:
        return hint.lower()
    lower = url.lower()
    if "bilibili.com" in lower or "b23.tv" in lower:
        return "bilibili"
    if "douyin.com" in lower or "iesdouyin.com" in lower:
        return "douyin"
    if "youtube.com" in lower or "youtu.be" in lower:
        return "youtube"
    return "other"


def normalize_douyin_url(url: str) -> str:
    lower = url.lower()
    if "douyin.com/jingxuan" not in lower:
        return url
    try:
        parsed = urlparse(url)
        query = parse_qs(parsed.query or "")
        modal_id = (query.get("modal_id") or query.get("modalId") or [None])[0]
        if modal_id and str(modal_id).isdigit():
            return f"https://www.douyin.com/video/{modal_id}"
    except Exception:
        return url
    return url


def build_output_template(output_dir: str) -> str:
    base = Path(output_dir).expanduser()
    base.mkdir(parents=True, exist_ok=True)
    return str(base / "%(title).80B_%(id)s.%(ext)s")


class SilentYdlLogger:
    def debug(self, msg: str) -> None:  # noqa: D401
        return

    def warning(self, msg: str) -> None:  # noqa: D401
        return

    def error(self, msg: str) -> None:  # noqa: D401
        return


def emit_progress(task_id: str, status: str, payload: Dict[str, Any]) -> None:
    downloaded = int(payload.get("downloaded_bytes") or 0)
    total = payload.get("total_bytes") or payload.get("total_bytes_estimate") or 0
    speed = payload.get("speed") or 0.0
    eta = payload.get("eta") or 0

    percent = 0.0
    if total and total > 0:
        percent = round(downloaded * 100.0 / float(total), 2)

    emit(
        "progress",
        {
            "task_id": task_id,
            "percent": percent,
            "speed": f"{speed / 1024 / 1024:.2f}MB/s" if speed else None,
            "eta": int(eta) if eta else None,
            "status": status,
            "message": payload.get("filename") or "",
        },
    )


def resolve_ffmpeg_path() -> Optional[str]:
    explicit = os.environ.get("FFMPEG_BINARY")
    if explicit and os.path.isfile(explicit):
        return explicit
    from_path = shutil.which("ffmpeg")
    if from_path:
        return from_path

    home = str(Path.home())
    local_app_data = os.environ.get("LOCALAPPDATA", "")
    candidates = [
        Path(home) / "Tools" / "ffmpeg" / "ffmpeg" / "ffmpeg-8.0.1-essentials_build" / "bin" / "ffmpeg.exe",
        Path(home) / "Tools" / "ffmpeg" / "bin" / "ffmpeg.exe",
        Path(home) / "scoop" / "shims" / "ffmpeg.exe",
        Path("C:/ffmpeg/bin/ffmpeg.exe"),
    ]
    if local_app_data:
        candidates.extend([
            Path(local_app_data) / "Microsoft" / "WinGet" / "Packages",
        ])

    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)

    if local_app_data:
        winget_root = Path(local_app_data) / "Microsoft" / "WinGet" / "Packages"
        if winget_root.exists():
            for ffmpeg in winget_root.glob("*ffmpeg*/*/ffmpeg.exe"):
                if ffmpeg.is_file():
                    return str(ffmpeg)

    return None


AUDIO_ONLY_FORMATS = {"mp3", "m4a", "flac"}


def build_format_selectors(
    ffmpeg_path: Optional[str], quality: str, fmt: str = "mp4", platform: str = "",
) -> list[str]:
    if fmt in AUDIO_ONLY_FORMATS:
        return ["bestaudio/best"]

    target_height = quality if quality.isdigit() else "1080"
    h = target_height

    if fmt == "video-only":
        return [
            f"bestvideo[height<={h}]/best[height<={h}]",
            "bestvideo/best",
        ]

    if ffmpeg_path:
        if fmt == "mp4" and platform == "youtube":
            return [
                f"bestvideo[height<={h}][vcodec^=avc1]+bestaudio[acodec^=mp4a]"
                f"/bestvideo[height<={h}][vcodec^=avc1]+bestaudio"
                f"/bestvideo[height<={h}]+bestaudio/best[height<={h}]",
                f"bv[height<={h}]+ba/best[height<={h}]",
                f"best[height<={h}]",
                "best",
            ]
        return [
            f"bestvideo[height<={h}]+bestaudio/best[height<={h}]",
            f"bv[height<={h}]+ba/best[height<={h}]",
            f"best[height<={h}]",
            "best",
        ]
    return [
        f"best[height<={h}]/best",
        "best",
    ]


def run_task(task: Dict[str, Any]) -> None:
    task_id = task["task_id"]
    start = time.time()
    quality = str(task.get("quality") or "").strip()
    output_dir = str(task.get("output_dir") or os.getcwd()).strip()
    fmt = task.get("format") or "mp4"
    cookie_path = (task.get("cookie_path") or "").strip()
    raw_url = str(task.get("url") or "")
    url = re.sub(r"\s+", "", raw_url).strip().strip('"').strip("'")
    if url != raw_url:
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "warn",
                "message": f"Normalize URL from [{raw_url}] to [{url}]",
            },
        )
    if not url:
        emit(
            "result",
            {
                "task_id": task_id,
                "success": False,
                "file_path": None,
                "duration_ms": 0,
                "error": "UNSUPPORTED_URL",
                "raw": "Missing URL",
            },
        )
        return

    douyin_normalized = normalize_douyin_url(url)
    if douyin_normalized != url:
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "info",
                "message": f"Normalize Douyin URL from [{url}] to [{douyin_normalized}]",
            },
        )
        url = douyin_normalized

    task["url"] = url
    platform = normalize_platform(url, task.get("platform", ""))
    task["platform"] = platform

    if platform == "other":
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "error",
                "message": f"Unsupported platform detected for URL: {url}",
            },
        )
        emit(
            "result",
            {
                "task_id": task_id,
                "success": False,
                "file_path": None,
                "duration_ms": 0,
                "error": "UNSUPPORTED_URL",
                "raw": f"Unsupported platform or URL: {url}",
            },
        )
        return

    ffmpeg_path = resolve_ffmpeg_path()
    emit(
        "log",
        {
            "task_id": task_id,
            "level": "info",
            "message": f"yt-dlp version: {YT_DLP_VERSION}",
        },
    )
    if ffmpeg_path:
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "info",
                "message": f"FFmpeg found: {ffmpeg_path}",
            },
        )
    else:
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "warn",
                "message": "FFmpeg not found, fallback to single-stream format",
            },
        )

    is_audio = fmt in AUDIO_ONLY_FORMATS
    is_video_only = fmt == "video-only"

    ydl_opts: Dict[str, Any] = {
        "outtmpl": build_output_template(output_dir),
    }

    if ffmpeg_path:
        ydl_opts["ffmpeg_location"] = ffmpeg_path
        if is_audio:
            ydl_opts["postprocessors"] = [{
                "key": "FFmpegExtractAudio",
                "preferredcodec": fmt,
                "preferredquality": "0" if fmt == "flac" else "192",
            }]
        elif is_video_only:
            ydl_opts["merge_output_format"] = "mp4"
        else:
            ydl_opts["merge_output_format"] = fmt
    elif is_audio:
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "error",
                "message": "FFmpeg is required for audio extraction but was not found. Please install FFmpeg.",
            },
        )

    ydl_opts.update({
        "cookiefile": None,
        "quiet": True,
        "no_warnings": True,
        "logger": SilentYdlLogger(),
        "extractor_retries": int(task.get("retry", 2) or 2),
        "retries": int(task.get("retry", 2) or 2),
        "socket_timeout": 30,
        "concurrent_fragment_downloads": 1,
        "noprogress": False,
        "writeinfojson": False,
    })

    ydl_opts = site_overrides(str(task.get("platform") or ""), ydl_opts)
    final_file: Optional[str] = None

    def progress_hook(d: Dict[str, Any]) -> None:
        nonlocal final_file
        status = d.get("status")
        if status == "downloading":
            emit_progress(task_id, "running", d)
        elif status == "finished":
            final_file = d.get("filename")
            emit_progress(task_id, "completed", d)
        elif status == "error":
            emit("log", {"task_id": task_id, "level": "warn", "message": d.get("error", "yt-dlp reported download error")})

    def resolve_final_path(info: Dict[str, Any], ydl: YoutubeDL) -> str:
        """Determine the actual on-disk file after any merging / post-processing."""
        nonlocal final_file
        requested = info.get("requested_downloads")
        if requested:
            path = requested[-1].get("filepath") or requested[-1].get("filename")
            if path and Path(path).exists():
                return path

        prepared = ydl.prepare_filename(info)

        if is_audio:
            audio_path = str(Path(prepared).with_suffix(f".{fmt}"))
            if Path(audio_path).exists():
                return audio_path

        if ffmpeg_path and fmt and fmt not in AUDIO_ONLY_FORMATS:
            target_ext = "mp4" if fmt == "video-only" else fmt
            merged = str(Path(prepared).with_suffix(f".{target_ext}"))
            if Path(merged).exists():
                return merged

        if Path(prepared).exists():
            return prepared

        if final_file and Path(final_file).exists():
            return final_file
        return prepared

    ydl_opts["progress_hooks"] = [progress_hook]
    ydl_opts = site_overrides(platform, ydl_opts)
    if cookie_path:
        ydl_opts["cookiefile"] = str(Path(cookie_path).expanduser())
        if not Path(cookie_path).exists():
            emit(
                "log",
                {
                    "task_id": task_id,
                    "level": "warn",
                    "message": f"Cookie file missing, ignored: {cookie_path}",
                },
            )
            ydl_opts["cookiefile"] = None
        else:
            emit(
                "log",
                {
                    "task_id": task_id,
                    "level": "info",
                    "message": f"Use cookie file: {cookie_path}",
                },
            )
    ydl_opts = {k: v for k, v in ydl_opts.items() if v is not None}
    if platform == "douyin" and not ydl_opts.get("cookiefile"):
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "warn",
                "message": "Douyin often requires fresh cookies; consider importing a new cookie file if download fails.",
            },
        )

    if ffmpeg_path and fmt == "mp4" and platform == "youtube":
        ydl_opts["postprocessor_args"] = {
            "merger": ["-c:v", "copy", "-c:a", "aac", "-b:a", "192k"],
        }

    format_selectors = build_format_selectors(ffmpeg_path, quality, fmt, platform)
    last_error: Optional[Exception] = None
    emit(
        "log",
        {
            "task_id": task_id,
            "level": "info",
            "message": f"Start downloading [{platform}] task",
        },
    )
    for selector in format_selectors:
        ydl_opts["format"] = selector
        emit(
            "log",
            {
                "task_id": task_id,
                "level": "info",
                "message": f"Try format selector: {selector}",
            },
        )
        try:
            with YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=True)
                actual_path = resolve_final_path(info, ydl)
                emit(
                    "result",
                    {
                        "task_id": task_id,
                        "success": True,
                        "file_path": actual_path,
                        "duration_ms": int((time.time() - start) * 1000),
                        "error": None,
                        "raw": None,
                    },
                )
                return
        except Exception as exc:
            last_error = exc
            message = str(exc)
            emit(
                "log",
                {
                    "task_id": task_id,
                    "level": "warn",
                    "message": f"format_selector failed ({selector}): {message}",
                },
            )
            if "requested format is not available" not in message.lower():
                break

    if last_error is not None:
        message = str(last_error)
        raw_message = message
        if "winerror 10013" in message.lower():
            raw_message = (
                f"{message} | HINT: Network policy, firewall, or proxy settings may block socket access "
                "(WinError 10013)."
            )
        elif "fresh cookies" in message.lower() or "cookies are needed" in message.lower():
            raw_message = (
                f"{message} | HINT: Douyin requires fresh browser cookies. Export and import a recent cookie file."
            )
        elif "requested format is not available" in message.lower() and not ffmpeg_path:
            raw_message = (
                f"{message} | HINT: No muxed stream may be available for this video. "
                "Install FFmpeg and retry so yt-dlp can merge video+audio streams."
            )
        emit(
            "result",
            {
                "task_id": task_id,
                "success": False,
                "file_path": None,
                "duration_ms": int((time.time() - start) * 1000),
                "error": classify_error(message, url),
                "raw": raw_message,
            },
        )


def main() -> int:
    configure_stdio()
    args = parse_args()
    try:
        payload = json.loads(args.task_json)
    except Exception as exc:
        emit(
            "result",
            {
                "task_id": "",
                "success": False,
                "file_path": None,
                "duration_ms": 0,
                "error": "NETWORK_FAIL",
                "raw": f"Invalid task payload: {exc}",
            },
        )
        return 2

    try:
        run_task(payload)
        return 0
    except KeyboardInterrupt:
        emit("log", {"task_id": payload.get("task_id", ""), "level": "warn", "message": "Download cancelled by user"})
        return 1
    except Exception as exc:  # noqa: BLE001
        emit(
            "result",
            {
                "task_id": payload.get("task_id", ""),
                "success": False,
                "file_path": None,
                "duration_ms": 0,
                "error": "NETWORK_FAIL",
                "raw": str(exc),
            },
        )
        return 2


if __name__ == "__main__":
    sys.exit(main())
