---
name: demo-video
description: Produce the DORMBIT Kiroween demo video (under 3:00) from screen recordings with ffmpeg, captions with whisper-cli, and checks with ffprobe. Use in Phase 15 when recording, editing, captioning, or checking the video.
---

# Demo video pipeline (Phase 15)

Approval gates: macOS Screen Recording permission, and making the video public, both need the user's OK first.

1. **Script** the beats from `docs/submission/` (problem, Find, Plan, Capture, Verify, Restore, Brief, what's real). Keep it under 2:50 to leave margin under the 3:00 limit.
2. **Record** at 2560×1600 or 1920×1200, 60 fps, into `media/raw/` (ignored by Git and by `.kiroignore`). Use the Dockline fixture only; no real folders, names, or emails on screen.
3. **Edit** with ffmpeg: cut, concatenate, normalize audio to −16 LUFS, export H.264 1080p, `-movflags +faststart`, to `media/final/dormbit-demo.mp4`.
4. **Captions**: `whisper-cli` on the final audio, fix the transcript by hand, burn in or ship as `.srt`.
5. **Check** with `ffprobe`: duration < 180 s, 1080p, audio present. Look at every frame range for home-folder paths, notifications, and third-party logos.
6. **Music**: none, or a track with a license that allows it. Record the license in `docs/submission/`.
7. Record the final duration and hash (`shasum -a 256`) in `docs/submission/checklist.md`.
