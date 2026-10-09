"""Focused local queue/security checks and an actual two-format FFmpeg render."""
from __future__ import annotations

import copy
import hashlib
import io
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import subprocess
import struct
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
import urllib.parse
import zlib
from unittest import mock

import engine
from server import StudioHTTPServer, parse_range


def campaign():
    return {
        "id": "test-campaign", "title": "瑪法冒險", "theme": "回到熟悉的世界", "description": "本地素材影片製作測試。",
        "claims": ["素材來自本地已標示來源。"], "cta": "關注旅程", "hashtags": ["#Mir2"], "requiredTags": ["bichon"],
        "shots": [
            {"seconds": 1, "headline": "回到瑪法", "caption": "素材來自本地已標示來源。", "narration": "", "assetRole": "creative"},
            {"seconds": 1, "headline": "熟悉的冒險", "caption": "本地素材影片製作測試。", "narration": "", "assetRole": "gameplay", "assetTags": ["bichon"], "factIndexes": [0]},
        ],
        "locales": {"en": {"title": "A Mir journey", "theme": "A familiar world", "description": "A local video render test.", "claims": ["Local assets have labelled sources."], "cta": "Follow the journey", "hashtags": ["#Mir2"], "shots": [
            {"seconds": 1, "headline": "Return to Mir", "caption": "Local assets have labelled sources.", "narration": "", "assetRole": "creative"},
            {"seconds": 1, "headline": "A familiar adventure", "caption": "A local video render test.", "narration": "", "assetRole": "gameplay"},
        ]}},
    }


def png_fixture():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 32, 32, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress((b"\x00" + b"\x18\x55\x72" * 32) * 32)) + chunk(b"IEND", b"")


class LocalFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="mir2-promo-test-")
        self.root = Path(self.temporary.name)
        self.campaign_file = self.root / "campaigns.json"
        self.campaign_file.write_text(json.dumps([campaign()], ensure_ascii=False), encoding="utf-8")
        self.studio = engine.Studio(self.root / "data", self.campaign_file, start_worker=False)

    def tearDown(self):
        self.studio.close()
        self.temporary.cleanup()

    def fake_asset(self, identifier, kind, tags=None):
        directory = self.studio.data_dir / "assets" / identifier
        directory.mkdir()
        target = directory / "source.png"
        target.write_bytes(b"0123456789" * 10)
        asset = {"id": identifier, "label": "fixture", "kind": "image", "tags": tags or [], "path": str(target), "sourceFileName": target.name, "provenance": {"kind": kind, "source": "test fixture"}, "thumbnailUrl": f"/media/{identifier}/thumbnail.jpg"}
        self.studio.assets[identifier] = asset
        return identifier

    def job(self):
        return {"campaignId": "test-campaign", "assetIds": ["a" * 32, "b" * 32], "formats": ["vertical", "landscape"], "language": "zh-TW", "voiceMode": "none"}


class TextAndProviderTests(unittest.TestCase):
    def test_ass_override_and_backslash_are_inert(self):
        text = engine.ass_text(r"{\pos(0,0)}danger\N" + "\nnormal")
        self.assertNotIn("{", text)
        self.assertNotIn("\\pos", text)
        self.assertTrue(text.endswith("\\Nnormal"))

    def test_rejects_control_characters(self):
        with self.assertRaises(engine.StudioError):
            engine.clean_text("title\x00bad", 80, "title")

    def test_local_regular_file_and_remote_rejection(self):
        for path in ("https://example.invalid/image.png", "\\\\server\\file.png", "relative.png"):
            with self.assertRaises(engine.StudioError):
                engine.local_regular_file(path)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "asset.png"
            source.write_bytes(b"test")
            self.assertEqual(engine.local_regular_file(str(source)), source.resolve())
            with mock.patch.object(Path, "is_symlink", return_value=True), self.assertRaises(engine.StudioError):
                engine.local_regular_file(str(source))

    def test_provider_cannot_introduce_claims_or_new_feature_headline(self):
        approved = engine.template_script(campaign(), "zh-TW")
        for field, value in (("claims", ["上線送999元寶"]), ("headline", "百萬玩家一起挑戰世界Boss")):
            forged = copy.deepcopy(approved)
            if field == "claims":
                forged[field] = value
            else:
                forged["shots"][0][field] = value
            with self.assertRaises(engine.StudioError):
                engine.validate_script(forged, approved=approved, provider=True)

    def test_new_emotional_hook_composes_only_vetted_phrases(self):
        approved = engine.template_script(campaign(), "zh-TW")
        script = copy.deepcopy(approved)
        script["shots"][0]["headline"] = "經典記憶，等你續寫！"
        result = engine.validate_script(script, approved=approved, provider=True)
        self.assertEqual(result["shots"][0]["headline"], script["shots"][0]["headline"])
        script["creativeTokens"] += ["上線送999元寶"]
        script["shots"][0]["headline"] = "上線送999元寶"
        with self.assertRaises(engine.StudioError):
            engine.validate_script(script, approved=approved, provider=True)

    def test_script_duration_schema_and_fact_indexes(self):
        approved = engine.template_script(campaign(), "zh-TW")
        for invalid in (float("nan"), 0, 16, True, "5"):
            script = copy.deepcopy(approved)
            script["shots"][0]["seconds"] = invalid
            with self.assertRaises(engine.StudioError):
                engine.validate_script(script)
        script = copy.deepcopy(approved)
        script["shots"][0]["factIndexes"] = [1]
        with self.assertRaises(engine.StudioError):
            engine.validate_script(script)

    def test_locale_override_and_timecodes(self):
        script = engine.template_script(campaign(), "en")
        self.assertEqual(script["title"], "A Mir journey")
        self.assertEqual(engine.timecode(61.234), "00:01:01,234")
        self.assertEqual(engine.timecode(61.23, ass=True), "0:01:01.23")

    def test_claim_indexes_cannot_be_rebound_by_deleting_or_reordering_facts(self):
        source = campaign()
        source["claims"] += ["第二條已驗證事實。", "第三條已驗證事實。"]
        approved = engine.template_script(source, "zh-TW")
        for claims in (list(reversed(approved["claims"])), approved["claims"][1:]):
            modified = copy.deepcopy(approved)
            modified["claims"] = claims
            modified["shots"][1]["factIndexes"] = [0]
            with self.assertRaisesRegex(engine.StudioError, "original order"):
                engine.validate_script(modified, approved=approved)

    def test_top_fields_and_shot_constraints_cannot_be_swapped(self):
        approved = engine.template_script(campaign(), "zh-TW")
        for field in ("title", "theme", "cta"):
            modified = copy.deepcopy(approved)
            modified[field] = approved["description"]
            with self.assertRaises(engine.StudioError):
                engine.validate_script(modified, approved=approved)
        for field, value in (("assetRole", "creative"), ("assetTags", []), ("shotId", "unknown"), ("caption", approved["shots"][0]["caption"])):
            modified = copy.deepcopy(approved)
            modified["shots"][1][field] = value
            with self.assertRaises(engine.StudioError):
                engine.validate_script(modified, approved=approved)
        # Omitting tags inherits trusted constraints and cannot remove them.
        modified = copy.deepcopy(approved)
        del modified["shots"][1]["assetTags"]
        self.assertEqual(engine.validate_script(modified, approved=approved)["shots"][1]["assetTags"], ["bichon"])
        translated = engine.template_script(campaign(), "en")
        self.assertEqual(translated["shots"][1]["shotId"], "2")
        self.assertEqual(translated["shots"][1]["assetTags"], ["bichon"])

    def test_trusted_fact_refs_cannot_be_changed_cleared_or_reordered(self):
        source = campaign()
        source["claims"] += ["第二條已驗證事實。", "第三條已驗證事實。"]
        source["shots"][1]["factIndexes"] = [0, 2]
        approved = engine.template_script(source, "zh-TW")
        for provider in (False, True):
            for indexes in ([], [2, 0], [0, 1], [0, 2, 1]):
                changed = copy.deepcopy(approved)
                changed["shots"][1]["factIndexes"] = indexes
                with self.assertRaisesRegex(engine.StudioError, "factIndexes"):
                    engine.validate_script(changed, approved=approved, provider=provider)
        unchanged = engine.validate_script(copy.deepcopy(approved), approved=approved)
        self.assertEqual(unchanged["shots"][1]["factIndexes"], [0, 2])
        # Inheritance follows shotId even after an allowed shot-order change.
        omitted = copy.deepcopy(approved)
        del omitted["shots"][1]["factIndexes"]
        omitted["shots"].reverse()
        inherited = engine.validate_script(omitted, approved=approved)
        by_id = {shot["shotId"]: shot["factIndexes"] for shot in inherited["shots"]}
        self.assertEqual(by_id, {"2": [0, 2], "1": []})

    def test_creative_shot_cannot_add_unapproved_fact_refs(self):
        approved = engine.template_script(campaign(), "zh-TW")
        self.assertEqual(approved["shots"][0]["factIndexes"], [])
        changed = copy.deepcopy(approved)
        changed["shots"][0]["factIndexes"] = [0]
        with self.assertRaisesRegex(engine.StudioError, "factIndexes"):
            engine.validate_script(changed, approved=approved, provider=True)
        del changed["shots"][0]["factIndexes"]
        inherited = engine.validate_script(changed, approved=approved, provider=True)
        self.assertEqual(inherited["shots"][0]["factIndexes"], [])


class PersistenceTests(LocalFixture):
    def setUp(self):
        super().setUp()
        self.fake_asset("a" * 32, "ai_creative")
        self.fake_asset("b" * 32, "game_screenshot", ["bichon"])
        self.studio.ffmpeg = self.studio.ffprobe = "fixture-tool"

    def test_queue_is_persistent_and_running_is_not_reexecuted(self):
        first = self.studio.enqueue(self.job())
        second = self.studio.enqueue(self.job())
        self.studio._update(second["id"], status="running", progress=10)
        restored = engine.Studio(self.studio.data_dir, self.campaign_file, start_worker=False)
        self.assertEqual(restored.jobs[first["id"]]["status"], "queued")
        self.assertEqual(restored.jobs[second["id"]]["status"], "error")
        self.assertIn("interrupted", restored.jobs[second["id"]]["error"])
        restored.close()
        self.assertEqual(self.studio.queue.qsize(), 2)

    def test_required_material_topics_and_roles_are_enforced(self):
        self.studio.assets["b" * 32]["tags"] = ["mining"]
        with self.assertRaisesRegex(engine.StudioError, "bichon"):
            self.studio.enqueue(self.job())
        self.studio.assets["b" * 32]["tags"] = ["bichon"]
        self.studio.assets["a" * 32]["provenance"]["kind"] = "game_screenshot"
        with self.assertRaisesRegex(engine.StudioError, "creative"):
            self.studio.enqueue(self.job())

    def test_media_never_serves_source_or_traversal(self):
        self.assertIsNone(self.studio.media_path("a" * 32, "../../state.json"))
        self.assertIsNone(self.studio.media_path("a" * 32, "state.json"))
        self.assertIsNone(self.studio.media_path("a" * 32, "source.png:stream"))
        self.assertIsNotNone(self.studio.media_path("a" * 32, "source.png"))

    def test_no_provider_key_is_transparent_template_mode(self):
        with mock.patch.dict(os.environ, {"PROMO_LLM_API_KEY": "", "PROMO_LLM_BASE_URL": "", "PROMO_LLM_MODEL": ""}):
            result = self.studio.brief({"campaignId": "test-campaign", "language": "en"})
        self.assertEqual(result["mode"], "template")
        self.assertFalse(result["configured"])
        self.assertIn("not a model", result["notice"])
        self.assertEqual(result["script"]["title"], "A Mir journey")

    def test_single_writer_lease_rejects_duplicate_studio(self):
        self.studio.start_worker()
        with self.assertRaisesRegex(engine.StudioError, "already has an active"):
            engine.Studio(self.studio.data_dir, self.campaign_file, start_worker=True)
        self.studio.close()
        replacement = engine.Studio(self.studio.data_dir, self.campaign_file, start_worker=True)
        replacement.close()

    def test_missing_shot_material_is_rejected_before_queueing(self):
        source = campaign()
        source["shots"][1]["assetTags"] = ["mining"]
        self.campaign_file.write_text(json.dumps([source], ensure_ascii=False), encoding="utf-8")
        with self.assertRaisesRegex(engine.StudioError, "mining"):
            self.studio.enqueue(self.job())
        self.assertEqual(self.studio.jobs, {})
        self.assertTrue(self.studio.queue.empty())

    @unittest.skipUnless(os.name == "nt", "System voice is Windows only")
    def test_empty_system_narration_is_rejected_explicitly(self):
        payload = self.job()
        payload["voiceMode"] = "system"
        with self.assertRaisesRegex(engine.StudioError, "Choose voiceMode none"):
            self.studio.enqueue(payload)

    def test_failed_enqueue_save_is_rolled_back_and_health_is_visible(self):
        with mock.patch("engine.os.replace", side_effect=OSError("simulated full disk")), self.assertRaises(engine.PersistenceError):
            self.studio.enqueue(self.job())
        self.assertEqual(self.studio.jobs, {})
        self.assertTrue(self.studio.queue.empty())
        self.assertEqual(self.studio.status()["health"]["storage"], "error")
        with self.assertRaises(engine.PersistenceError):
            self.studio.enqueue(self.job())

    def test_completion_save_failure_keeps_worker_alive_and_never_claims_success(self):
        first = self.studio.enqueue(self.job())
        second = self.studio.enqueue(self.job())
        replace = os.replace
        def fail_completed(source, target):
            candidate = json.loads(Path(source).read_text(encoding="utf-8"))
            if any(j["status"] == "completed" for j in candidate["jobs"]):
                raise OSError("simulated full disk at completion commit")
            return replace(source, target)
        with mock.patch("engine.os.replace", side_effect=fail_completed), mock.patch.object(self.studio, "render", return_value=[{"format": "vertical", "videoUrl": "uncommitted.mp4"}]):
            self.studio.start_worker()
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and (self.studio.status()["health"]["storage"] != "error" or self.studio.jobs[first["id"]]["status"] != "error"):
                time.sleep(.02)
            status = self.studio.status()
            self.assertTrue(status["health"]["workerAlive"])
            self.assertEqual(status["health"]["worker"], "degraded")
            self.assertEqual(self.studio.jobs[first["id"]]["status"], "error")
            self.assertEqual(self.studio.jobs[first["id"]]["outputs"], [])
            self.assertEqual(self.studio.jobs[second["id"]]["status"], "queued")
            self.assertIn("restart", self.studio.jobs[second["id"]]["blockedReason"])
            with self.assertRaises(engine.PersistenceError):
                self.studio.enqueue(self.job())
            persisted = json.loads(self.studio.state_path.read_text(encoding="utf-8"))
            self.assertFalse(any(j["status"] == "completed" for j in persisted["jobs"]))
            self.assertEqual(list(self.studio.data_dir.glob(".state-*.tmp")), [])
        self.studio.close()
        restored = engine.Studio(self.studio.data_dir, self.campaign_file, start_worker=False)
        self.assertEqual(restored.jobs[first["id"]]["status"], "error")
        self.assertEqual(restored.jobs[second["id"]]["status"], "queued")
        self.assertEqual(restored.status()["health"]["storage"], "ok")
        restored.close()

    def test_error_handler_save_failure_does_not_kill_worker(self):
        job = self.studio.enqueue(self.job())
        replace = os.replace
        def fail_error(source, target):
            candidate = json.loads(Path(source).read_text(encoding="utf-8"))
            if any(j["status"] == "error" for j in candidate["jobs"]):
                raise OSError("simulated failing error-state commit")
            return replace(source, target)
        with mock.patch("engine.os.replace", side_effect=fail_error), mock.patch.object(self.studio, "render", side_effect=engine.StudioError("test decoder failure")):
            self.studio.start_worker()
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and (self.studio.status()["health"]["storage"] != "error" or self.studio.jobs[job["id"]]["status"] != "error"):
                time.sleep(.02)
            self.assertTrue(self.studio.worker.is_alive())
            self.assertEqual(self.studio.jobs[job["id"]]["status"], "error")
            self.assertEqual(self.studio.status()["health"]["storage"], "error")


class ConfiguredProviderTests(LocalFixture):
    def test_provider_accepted_hook_and_key_never_in_status_or_state(self):
        script = engine.template_script(campaign(), "zh-TW")
        script["shots"][0]["headline"] = "經典記憶，等你續寫！"
        script["shots"][1]["factIndexes"] = [0]
        response = {"choices": [{"message": {"content": json.dumps(script, ensure_ascii=False)}}]}
        opener = mock.MagicMock()
        opener.open.return_value = io.BytesIO(json.dumps(response).encode())
        with mock.patch.dict(os.environ, {"PROMO_LLM_API_KEY": "test-secret-never-display", "PROMO_LLM_BASE_URL": "https://provider.invalid/v1", "PROMO_LLM_MODEL": "test-model"}), mock.patch("engine.urllib.request.build_opener", return_value=opener):
            result = self.studio.brief({"campaignId": "test-campaign", "theme": "A fresh nostalgic opening"})
            status = self.studio.status()
        self.assertEqual(result["mode"], "provider")
        self.assertEqual(result["script"]["shots"][0]["headline"], script["shots"][0]["headline"])
        self.assertNotIn("test-secret-never-display", json.dumps(status))
        self.assertFalse(self.studio.state_path.exists())
        handler = engine.NoProviderRedirect()
        self.assertIsNone(handler.redirect_request(None, None, 302, "redirect", None, "https://other.invalid"))

    def test_provider_unverified_facts_and_nonjson_response_rejected(self):
        script = engine.template_script(campaign(), "zh-TW")
        script["shots"][1]["narration"] = "人人免費領取999元寶"
        for content in (json.dumps(script), "not json"):
            opener = mock.MagicMock()
            opener.open.return_value = io.BytesIO(json.dumps({"choices": [{"message": {"content": content}}]}).encode())
            with mock.patch.dict(os.environ, {"PROMO_LLM_API_KEY": "test-secret", "PROMO_LLM_BASE_URL": "https://provider.invalid/v1", "PROMO_LLM_MODEL": "test-model"}), mock.patch("engine.urllib.request.build_opener", return_value=opener), self.assertRaises(engine.StudioError):
                self.studio.brief({"campaignId": "test-campaign"})

    def test_nonlocal_plaintext_provider_is_rejected(self):
        with mock.patch.dict(os.environ, {"PROMO_LLM_API_KEY": "test-secret", "PROMO_LLM_BASE_URL": "http://provider.invalid/v1", "PROMO_LLM_MODEL": "test-model"}), self.assertRaisesRegex(engine.StudioError, "HTTPS"):
            self.studio.brief({"campaignId": "test-campaign"})


@unittest.skipUnless(engine.executable("ffmpeg") and engine.executable("ffprobe"), "Real media tools required")
class MediaSafetyTests(LocalFixture):
    def test_disguised_playlists_are_rejected_before_decoder_or_external_read(self):
        hits = {"count": 0}
        class Trap(BaseHTTPRequestHandler):
            def do_GET(self):
                hits["count"] += 1
                self.send_response(200)
                self.send_header("Content-Length", "0")
                self.end_headers()
            def log_message(self, format, *args):
                pass
        trap = HTTPServer(("127.0.0.1", 0), Trap)
        thread = threading.Thread(target=trap.serve_forever, daemon=True)
        thread.start()
        protected = self.root / "protected-never-read.bin"
        protected.write_bytes(b"must not be opened by media inspection")
        playlist = f"#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nhttp://127.0.0.1:{trap.server_port}/segment.ts\n#EXTINF:1,\n{protected.as_uri()}\n#EXT-X-ENDLIST\n".encode()
        open_file = Path.open
        def guarded_open(path, *args, **kwargs):
            if path == protected:
                raise AssertionError("External local reference was read")
            return open_file(path, *args, **kwargs)
        try:
            with mock.patch("engine.subprocess.run", wraps=subprocess.run) as decoder, mock.patch.object(Path, "open", guarded_open):
                for suffix in sorted(engine.IMAGE_EXTENSIONS | engine.VIDEO_EXTENSIONS):
                    source = self.root / ("disguised" + suffix)
                    source.write_bytes(playlist)
                    with self.assertRaisesRegex(engine.StudioError, "File bytes"):
                        self.studio.import_asset({"path": str(source), "label": "not media", "provenance": {"kind": "ai_creative", "source": "negative playlist test"}})
                    with self.assertRaisesRegex(engine.StudioError, "File bytes"):
                        engine.probe(source, self.studio.ffprobe)
                decoder.assert_not_called()
            # Even a forged ftyp prefix is read only by the fixed MOV demuxer,
            # with external data references explicitly disabled.
            source = self.root / "polyglot.mp4"
            source.write_bytes(struct.pack(">I", 24) + b"ftypisom\x00\x00\x02\x00isomiso2" + playlist)
            with self.assertRaises(engine.StudioError):
                self.studio.import_asset({"path": str(source), "label": "not MOV media", "provenance": {"kind": "ai_creative", "source": "negative fixed-demuxer test"}})
            self.assertEqual(hits["count"], 0)
            self.assertEqual(list((self.studio.data_dir / "assets").iterdir()), [])
        finally:
            trap.shutdown()
            trap.server_close()
            thread.join(timeout=2)

    def test_magic_and_safe_demuxer_options_cover_supported_types(self):
        headers = {".png": b"\x89PNG\r\n\x1a\n", ".jpg": b"\xff\xd8\xff", ".jpeg": b"\xff\xd8\xff", ".webp": b"RIFF0000WEBP", ".bmp": b"BM", ".gif": b"GIF89a", ".avi": b"RIFF0000AVI ", ".mkv": b"\x1a\x45\xdf\xa3", ".webm": b"\x1a\x45\xdf\xa3", ".mp4": struct.pack(">I", 16) + b"ftypisom0000", ".mov": struct.pack(">I", 16) + b"ftypqt  0000", ".m4v": struct.pack(">I", 16) + b"ftypisom0000"}
        for suffix, header in headers.items():
            source = self.root / ("header" + suffix)
            source.write_bytes(header)
            options = engine.media_input_options(source)
            self.assertEqual(options[options.index("-protocol_whitelist") + 1], "file,pipe")
            self.assertEqual(options[options.index("-f") + 1], engine.DEMUXERS[suffix])
            self.assertIn("-format_whitelist", options)
            if suffix in (".mp4", ".mov", ".m4v"):
                self.assertEqual(options[options.index("-enable_drefs") + 1], "0")
                self.assertEqual(options[options.index("-use_absolute_path") + 1], "0")

    def test_failed_import_cleans_only_its_owned_copy_and_preserves_source(self):
        source = self.root / "original.png"
        source.write_bytes(png_fixture())
        original = source.read_bytes()
        with mock.patch.object(self.studio, "_run", side_effect=engine.StudioError("simulated thumbnail failure")), self.assertRaises(engine.StudioError):
            self.studio.import_asset({"path": str(source), "label": "valid source", "provenance": {"kind": "game_screenshot", "source": "cleanup fixture"}})
        self.assertEqual(source.read_bytes(), original)
        self.assertEqual(list((self.studio.data_dir / "assets").iterdir()), [])
        unrelated = self.root / ("d" * 32)
        unrelated.mkdir()
        witness = unrelated / "preserve.bin"
        witness.write_bytes(b"preserve unrelated work")
        with self.assertRaises(OSError):
            self.studio._cleanup_failed_import(unrelated, "d" * 32)
        self.assertEqual(witness.read_bytes(), b"preserve unrelated work")

    def test_import_state_save_failure_removes_uncommitted_asset(self):
        source = self.root / "source.png"
        source.write_bytes(png_fixture())
        with mock.patch("engine.os.replace", side_effect=OSError("simulated state write failure")), self.assertRaises(engine.PersistenceError):
            self.studio.import_asset({"path": str(source), "label": "save failure", "provenance": {"kind": "game_screenshot", "source": "fault fixture"}})
        self.assertEqual(self.studio.assets, {})
        self.assertEqual(list((self.studio.data_dir / "assets").iterdir()), [])
        self.assertEqual(source.read_bytes(), png_fixture())


class HTTPTests(LocalFixture):
    def setUp(self):
        super().setUp()
        self.fake_asset("a" * 32, "ai_creative")
        self.fake_asset("b" * 32, "game_screenshot", ["bichon"])
        self.studio.ffmpeg = self.studio.ffprobe = "fixture-tool"
        self.http = StudioHTTPServer(("127.0.0.1", 0), self.studio)
        self.thread = threading.Thread(target=self.http.serve_forever, daemon=True)
        self.thread.start()
        self.base = f"http://127.0.0.1:{self.http.server_port}"

    def tearDown(self):
        self.http.shutdown()
        self.http.server_close()
        self.thread.join(timeout=2)
        super().tearDown()

    def request(self, path, payload=None, headers=None):
        defaults = {"Origin": self.base, "Content-Type": "application/json"}
        defaults.update(headers or {})
        return urllib.request.urlopen(urllib.request.Request(self.base + path, data=json.dumps(payload).encode() if payload is not None else None, headers=defaults), timeout=5)

    def test_host_and_origin_reject_cross_site_mutation(self):
        for headers in ({"Host": "attacker.invalid"}, {"Origin": "https://attacker.invalid"}, {"Origin": "null"}):
            with self.assertRaises(urllib.error.HTTPError) as failure:
                self.request("/api/jobs", self.job(), headers)
            self.assertEqual(failure.exception.code, 403)
        with self.request("/api/jobs", self.job()) as response:
            self.assertEqual(response.status, 202)
            self.assertEqual(json.load(response)["status"], "queued")

    def test_media_single_range_and_unsatisfiable_response(self):
        with self.request("/media/" + "a" * 32 + "/source.png", headers={"Range": "bytes=10-19"}) as response:
            self.assertEqual(response.status, 206)
            self.assertEqual(response.read(), b"0123456789")
            self.assertEqual(response.headers["Content-Range"], "bytes 10-19/100")
        with self.assertRaises(urllib.error.HTTPError) as failure:
            self.request("/media/" + "a" * 32 + "/source.png", headers={"Range": "bytes=500-600"})
        self.assertEqual(failure.exception.code, 416)
        with self.assertRaises(urllib.error.HTTPError):
            self.request("/media/" + "a" * 32 + "/%2e%2e%2fstate.json")

    def test_finite_json_only(self):
        payload = self.job()
        payload["unused"] = float("nan")
        with self.assertRaises(urllib.error.HTTPError) as failure:
            self.request("/api/jobs", payload)
        self.assertEqual(failure.exception.code, 400)

    def test_status_contract_includes_retry_script_and_assets_without_source_paths(self):
        self.studio.assets["a" * 32]["sourcePath"] = "private-source-must-not-be-returned"
        submitted = self.studio.enqueue(self.job())
        self.studio._update(submitted["id"], status="running")
        with self.request("/api/status") as response:
            status = json.load(response)
        job = status["jobs"][0]
        self.assertEqual(job["status"], "running")
        self.assertEqual(job["assetIds"], self.job()["assetIds"])
        self.assertEqual(job["script"]["shots"][0]["shotId"], "1")
        self.assertEqual(job["script"]["shots"][1]["assetTags"], ["bichon"])
        self.assertNotIn("private-source-must-not-be-returned", json.dumps(status))
        self.assertIn("health", status)

    def test_browser_binary_upload_is_streamed_validated_and_temp_is_cleaned(self):
        self.studio.ffmpeg, self.studio.ffprobe = engine.executable("ffmpeg"), engine.executable("ffprobe")
        body = png_fixture()
        query = urllib.parse.urlencode({"filename": "browser.png", "label": "Browser fixture", "kind": "game_screenshot", "source": "Synthetic upload test fixture", "tags": "bichon"})
        request = urllib.request.Request(self.base + "/api/upload?" + query, data=body, headers={"Origin": self.base, "Content-Type": "application/octet-stream"})
        with urllib.request.urlopen(request, timeout=10) as response:
            self.assertEqual(response.status, 201)
            asset = json.load(response)
        self.assertEqual(asset["tags"], ["bichon"])
        self.assertEqual(asset["label"], "Browser fixture")
        self.assertEqual(asset["sourceFileName"], "browser.png")
        self.assertEqual(Path(asset["path"]).read_bytes(), body)
        self.assertEqual(list((self.studio.data_dir / "uploads").iterdir()), [])
        wrong = urllib.request.Request(self.base + "/api/upload?" + query, data=b"not a PNG", headers={"Origin": self.base, "Content-Type": "application/octet-stream"})
        with self.assertRaises(urllib.error.HTTPError) as failure:
            urllib.request.urlopen(wrong, timeout=10)
        self.assertEqual(failure.exception.code, 400)
        self.assertEqual(list((self.studio.data_dir / "uploads").iterdir()), [])

    def test_upload_rejects_cross_origin_size_and_filename_escape(self):
        for query, headers, expected in (
            ({"filename": "../escape.png", "kind": "ai_creative"}, {}, 400),
            ({"filename": "test.png", "kind": "ai_creative"}, {"Origin": "https://attacker.invalid"}, 403),
            ({"filename": "test.png", "kind": "ai_creative"}, {"Content-Length": str(engine.MAX_ASSET_BYTES + 1)}, 413),
            ({"filename": "test.png", "kind": "ai_creative"}, {"Content-Type": "image/png"}, 415),
        ):
            defaults = {"Origin": self.base, "Content-Type": "application/octet-stream"}
            defaults.update(headers)
            request = urllib.request.Request(self.base + "/api/upload?" + urllib.parse.urlencode(query), data=b"x", headers=defaults)
            with self.assertRaises(urllib.error.HTTPError) as failure:
                urllib.request.urlopen(request, timeout=5)
            self.assertEqual(failure.exception.code, expected)


class RangeTests(unittest.TestCase):
    def test_open_suffix_and_end_clamping(self):
        self.assertEqual(parse_range("bytes=3-", 10), (3, 9))
        self.assertEqual(parse_range("bytes=-4", 10), (6, 9))
        self.assertEqual(parse_range("bytes=0-1000", 10), (0, 9))
        self.assertIsNone(parse_range(None, 10))
        for value in ("bytes=-0", "bytes=-", "bytes=8-4", "bytes=0-1,4-5", "items=1-5"):
            with self.assertRaises(engine.StudioError):
                parse_range(value, 10)


class SourceTimingTests(unittest.TestCase):
    def test_asset_selection_matches_every_class_tag_and_never_creative_role(self):
        def asset(identifier, tags, kind="gameplay"):
            return {"id": identifier, "provenance": {"kind": kind}, "tags": tags}
        assets = [asset("mining", ["mining"]), asset("warrior-one", ["classes", "warrior"]), asset("wizard", ["classes", "wizard"]), asset("taoist", ["classes", "taoist"]), asset("warrior-two", ["classes", "warrior"]), asset("warrior-art", ["classes", "warrior"], "ai_creative"), asset("incomplete-warrior-tag", ["warrior"])]
        shots = [{"shotId": str(i+1), "assetRole": "gameplay", "assetTags": ["classes", topic]} for i, topic in enumerate(("warrior", "wizard", "taoist", "warrior"))]
        self.assertEqual([a["id"] for a in engine.choose_shot_assets(shots, assets)], ["warrior-one", "wizard", "taoist", "warrior-two"])

    def test_repeated_clip_continues_then_truthfully_records_looping(self):
        shots = [{"seconds": 6} for _ in range(5)]
        video = {"id": "a" * 32, "kind": "video", "durationSeconds": 22}
        timings = engine.source_timing(shots, [video] * 5)
        self.assertEqual([t["inputOffsetSeconds"] for t in timings], [0, 6, 12, 18, 2])
        self.assertEqual([t["sourceLooped"] for t in timings], [False, False, False, True, True])
        self.assertTrue(all(t["inputOffsetSeconds"] < 22 for t in timings))

    def test_each_source_has_independent_cursor_and_stills_do_not_seek(self):
        first = {"id": "a" * 32, "kind": "video", "durationSeconds": 12}
        second = {"id": "b" * 32, "kind": "video", "durationSeconds": 12}
        still = {"id": "c" * 32, "kind": "image"}
        timings = engine.source_timing([{"seconds": 2}] * 5, [first, second, still, first, second])
        self.assertEqual([t["inputOffsetSeconds"] for t in timings], [0, 0, 0, 2, 2])
        self.assertFalse(any(t["sourceLooped"] for t in timings))


@unittest.skipUnless(engine.executable("ffmpeg") and engine.executable("ffprobe"), "FFmpeg/ffprobe are required for the real render acceptance")
class ActualFFmpegTests(LocalFixture):
    def test_supported_image_and_video_types_decode_with_fixed_demuxers(self):
        for suffix in (".png", ".jpg", ".webp", ".bmp", ".gif"):
            source = self.root / ("actual" + suffix)
            subprocess.run([self.studio.ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0x406070:s=64x64", "-frames:v", "1", "-threads", "1", str(source)], check=True, capture_output=True, timeout=30)
            imported = self.studio.import_asset({"path": str(source), "label": "Actual image container test", "provenance": {"kind": "game_screenshot", "source": "Synthetic test fixture " + suffix}})
            self.assertEqual((imported["width"], imported["height"]), (64, 64))
        for suffix, codec in ((".mp4", "libx264"), (".mov", "libx264"), (".mkv", "libx264"), (".avi", "mpeg4"), (".webm", "libvpx-vp9")):
            source = self.root / ("actual" + suffix)
            subprocess.run([self.studio.ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0x406070:s=64x64:r=30", "-t", "1", "-c:v", codec, "-pix_fmt", "yuv420p", "-threads", "1", str(source)], check=True, capture_output=True, timeout=30)
            imported = self.studio.import_asset({"path": str(source), "label": "Actual video container test", "provenance": {"kind": "gameplay", "source": "Synthetic test fixture " + suffix}})
            self.assertEqual((imported["width"], imported["height"]), (64, 64))
            self.assertGreater(imported["durationSeconds"], .9)

    def test_full_creative_background_motion_and_continuing_gameplay_frames(self):
        source = self.root / "portrait.png"
        subprocess.run([self.studio.ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0x406070:s=360x640,drawbox=x=150:y=200:w=30:h=300:color=white:t=fill", "-frames:v", "1", "-threads", "1", str(source)], check=True, capture_output=True, timeout=30)
        creative = self.studio.import_asset({"path": str(source), "label": "Portrait motion fixture", "provenance": {"kind": "ai_creative", "source": "Synthetic portrait; test-only"}})
        video_source = self.root / "timeline.mp4"
        args = [self.studio.ffmpeg, "-v", "error", "-y"]
        for color in ("red", "green", "blue"):
            args += ["-f", "lavfi", "-i", f"color=c={color}:s=320x180:r=30:d=1"]
        args += ["-filter_complex", "[0:v][1:v][2:v]concat=n=3:v=1:a=0,drawbox=x=0:y=0:w=iw:h=ih:t=8:color=yellow,format=yuv420p[v]", "-map", "[v]", "-c:v", "libx264", "-threads", "1", str(video_source)]
        subprocess.run(args, check=True, capture_output=True, timeout=30)
        video = self.studio.import_asset({"path": str(video_source), "label": "Red-green-blue timeline", "tags": ["bichon"], "provenance": {"kind": "gameplay", "source": "Synthetic timeline with a full-frame yellow border; test-only"}})
        updated = campaign()
        updated["shots"] += [copy.deepcopy(updated["shots"][1]), copy.deepcopy(updated["shots"][1])]
        self.campaign_file.write_text(json.dumps([updated], ensure_ascii=False), encoding="utf-8")
        job = self.studio.enqueue({"campaignId": "test-campaign", "assetIds": [creative["id"], video["id"]], "formats": ["vertical"], "language": "zh-TW", "voiceMode": "none"})
        self.studio.start_worker()
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            current = self.studio.status()["jobs"][0]
            if current["status"] not in ("queued", "running"):
                break
            time.sleep(.2)
        self.assertEqual(current["status"], "completed", current.get("error", "render timeout"))
        output = current["outputs"][0]
        target = self.studio.media_path(job["id"], Path(output["videoUrl"]).name)
        def pixels(at, x, y, width=1):
            result = subprocess.run([self.studio.ffmpeg, "-v", "error", "-ss", str(at), "-i", str(target), "-vf", f"format=rgb24,crop={width}:1:{x}:{y}", "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], check=True, capture_output=True, timeout=15)
            return result.stdout
        # Artwork reaches the left edge instead of occupying a central box.
        portrait_edge = pixels(.5, 20, 640)
        self.assertGreater(min(portrait_edge), 35)
        early, late = pixels(.25, 280, 700, 100), pixels(.65, 280, 700, 100)
        first_bright = lambda row: next(i for i in range(100) if row[i*3] > 200)
        self.assertNotEqual(first_bright(early), first_bright(late), "Ken Burns should visibly move the portrait's stripe")
        # The three consecutive shots consume red, green, then blue footage.
        for at, dominant in ((1.5, 0), (2.5, 1), (3.5, 2)):
            color = pixels(at, 360, 640)
            self.assertEqual(max(range(3), key=lambda n: color[n]), dominant)
        # A border representing edge HUD remains visible on all four sides.
        for x, y in ((28, 640), (690, 640), (360, 450), (360, 817)):
            border = pixels(3.5, x, y)
            self.assertGreater(border[0], 180)
            self.assertGreater(border[1], 160)
            self.assertLess(border[2], 100)
        metadata = json.loads(self.studio.media_path(job["id"], Path(output["metadataUrl"]).name).read_text(encoding="utf-8"))
        shots = metadata["assetProvenance"]
        self.assertEqual(shots[0]["presentation"]["fit"], "creative_full_frame")
        self.assertEqual(shots[0]["presentation"]["motion"], "ken_burns_1.00_to_1.03")
        self.assertEqual([s["inputOffsetSeconds"] for s in shots[1:]], [0, 1, 2])
        self.assertFalse(any(s["sourceLooped"] for s in shots))
        self.assertFalse(any(s["presentation"]["gameplayCropApplied"] for s in shots))

    def test_real_vertical_landscape_unicode_and_source_preservation(self):
        ffmpeg = self.studio.ffmpeg
        source = self.root / "source.png"
        subprocess.run([ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=320x180:r=30", "-frames:v", "1", "-threads", "1", str(source)], check=True, capture_output=True, timeout=30)
        original_hash = hashlib.sha256(source.read_bytes()).hexdigest()
        creative = self.studio.import_asset({"path": str(source), "label": "Synthetic creative test fixture", "provenance": {"kind": "ai_creative", "source": "FFmpeg test pattern; test-only provenance fixture"}})
        screenshot = self.studio.import_asset({"path": str(source), "label": "Synthetic still test fixture", "tags": ["bichon"], "provenance": {"kind": "game_screenshot", "source": "FFmpeg test pattern; not actual Mir2 promotional material"}})
        video_source = self.root / "source.mp4"
        subprocess.run([ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=320x180:r=30", "-t", "1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-threads", "1", str(video_source)], check=True, capture_output=True, timeout=30)
        video = self.studio.import_asset({"path": str(video_source), "label": "Synthetic video test fixture", "tags": ["bichon"], "provenance": {"kind": "gameplay", "source": "FFmpeg generated video fixture; not actual Mir2 promotional material"}})
        updated_campaign = campaign()
        updated_campaign["shots"].append(copy.deepcopy(updated_campaign["shots"][1]))
        self.campaign_file.write_text(json.dumps([updated_campaign], ensure_ascii=False), encoding="utf-8")
        with self.assertRaisesRegex(engine.StudioError, "real video"):
            self.studio.import_asset({"path": str(source), "label": "wrong", "provenance": {"kind": "gameplay", "source": "test"}})
        job = self.studio.enqueue({"campaignId": "test-campaign", "assetIds": [creative["id"], screenshot["id"], video["id"]], "formats": ["vertical", "landscape"], "language": "zh-TW", "voiceMode": "none"})
        self.studio.start_worker()
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            current = self.studio.status()["jobs"][0]
            if current["status"] not in ("queued", "running"):
                break
            time.sleep(.2)
        self.assertEqual(current["status"], "completed", current.get("error", "render timeout"))
        self.assertEqual(current["progress"], 100)
        self.assertEqual(len(current["outputs"]), 2)
        for output in current["outputs"]:
            target = self.studio.media_path(job["id"], Path(output["videoUrl"]).name)
            info = engine.probe(target, self.studio.ffprobe)
            self.assertEqual((info["width"], info["height"]), engine.FORMATS[output["format"]])
            self.assertEqual(info["codec"], "h264")
            self.assertGreater(info["durationSeconds"], 2.95)
            self.assertLess(info["durationSeconds"], 3.2)
            codec = subprocess.run([self.studio.ffprobe, "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=pix_fmt,r_frame_rate", "-of", "json", str(target)], capture_output=True, check=True, timeout=10)
            details = json.loads(codec.stdout)["streams"][0]
            self.assertEqual(details["pix_fmt"], "yuv420p")
            self.assertEqual(details["r_frame_rate"], "30/1")
            metadata = json.loads(self.studio.media_path(job["id"], Path(output["metadataUrl"]).name).read_text(encoding="utf-8"))
            self.assertEqual(set(metadata["disclosures"]), {"ai_creative", "game_screenshot", "gameplay"})
            subtitle = self.studio.media_path(job["id"], Path(output["subtitleUrl"]).name).read_text(encoding="utf-8")
            self.assertIn("00:00:01,000", subtitle)
            self.assertIn("素材", subtitle)
            self.assertGreater(self.studio.media_path(job["id"], Path(output["coverUrl"]).name).stat().st_size, 1000)
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), original_hash)
        self.assertIn("截圖演示", (self.studio.data_dir / "jobs" / job["id"] / "segments-vertical/scene-1.ass").read_text(encoding="utf-8"))

    @unittest.skipUnless(os.name == "nt", "Windows system narration is only available on Windows")
    def test_real_system_tts_produces_audible_video_and_honest_metadata(self):
        source = self.root / "source.png"
        subprocess.run([self.studio.ffmpeg, "-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0x245360:s=320x180", "-frames:v", "1", "-threads", "1", str(source)], check=True, capture_output=True, timeout=30)
        creative = self.studio.import_asset({"path": str(source), "label": "test creative", "provenance": {"kind": "ai_creative", "source": "Synthetic test-only image"}})
        screenshot = self.studio.import_asset({"path": str(source), "label": "test screenshot", "tags": ["bichon"], "provenance": {"kind": "game_screenshot", "source": "Synthetic test-only image"}})
        updated_campaign = campaign()
        updated_campaign["shots"][0]["narration"] = "回到瑪法"
        self.campaign_file.write_text(json.dumps([updated_campaign], ensure_ascii=False), encoding="utf-8")
        job = self.studio.enqueue({"campaignId": "test-campaign", "assetIds": [creative["id"], screenshot["id"]], "formats": ["vertical"], "language": "zh-TW", "voiceMode": "system"})
        self.studio.start_worker()
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            current = self.studio.status()["jobs"][0]
            if current["status"] not in ("queued", "running"):
                break
            time.sleep(.2)
        self.assertEqual(current["status"], "completed", current.get("error", "render timeout"))
        output = current["outputs"][0]
        metadata = json.loads(self.studio.media_path(job["id"], Path(output["metadataUrl"]).name).read_text(encoding="utf-8"))
        self.assertIn("system TTS", metadata["voice"]["engine"])
        self.assertIn("not neural AI", metadata["voice"]["engine"])
        self.assertTrue(metadata["voice"]["installedVoices"])
        decoded = subprocess.run([self.studio.ffmpeg, "-v", "error", "-i", str(self.studio.media_path(job["id"], Path(output["videoUrl"]).name)), "-map", "0:a:0", "-ac", "1", "-ar", "8000", "-f", "f32le", "-"], capture_output=True, check=True, timeout=15)
        import array
        samples = array.array("f")
        samples.frombytes(decoded.stdout)
        self.assertGreater(max(abs(sample) for sample in samples), .01)


if __name__ == "__main__":
    unittest.main(verbosity=2)
