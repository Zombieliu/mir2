"""Check the shared ICO and optional built Windows EXEs without running them.

Usage: python verify-windows-icons.py [--exe path/to/Mir2Launcher.exe ...]
       python verify-windows-icons.py --setup path/to/Setup.exe
The checked-in game icon remains the single source for launcher, updater,
native game and setup branding. EXE checks require Windows.
"""
import argparse
import ctypes
import json
import pathlib
import struct
import sys
from ctypes import wintypes


ICON = pathlib.Path(__file__).resolve().parents[2] / "platform-windows/resources/numeron.ico"
REQUIRED_SIZES = {16, 24, 32, 48, 64, 256}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_ico(path):
    raw = path.read_bytes()
    require(len(raw) >= 6, "truncated ICO header")
    reserved, kind, count = struct.unpack_from("<HHH", raw)
    require((reserved, kind, count) == (0, 1, len(REQUIRED_SIZES)), "expected six ICO frames")
    require(len(raw) >= 6 + 16 * count, "truncated ICO directory")
    frames = []
    for index in range(count):
        entry = struct.unpack_from("<BBBBHHII", raw, 6 + 16 * index)
        width, height, colors, zero, planes, bits, size, offset = entry
        width, height = width or 256, height or 256
        require(width == height and bits == 32 and zero == 0, f"invalid ICO frame {index}")
        require(offset >= 6 + 16 * count and offset + size <= len(raw), "ICO payload outside file")
        payload = raw[offset:offset + size]
        require(payload[:8] == b"\x89PNG\r\n\x1a\n" and len(payload) >= 33, "expected PNG ICO frame")
        png_width, png_height, depth, color = struct.unpack_from(">IIBB", payload, 16)
        require((png_width, png_height, depth, color) == (width, height, 8, 6), "ICO must contain RGBA PNG frames")
        frames.append((entry[:6], payload))
    require({(entry[0] or 256) for entry, _ in frames} == REQUIRED_SIZES, "required ICO sizes missing")
    return frames


def verify_exe(path, frames, group_id=1):
    require(sys.platform == "win32", "EXE resource checks require Windows")
    path = path.resolve(strict=True)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = wintypes.HMODULE
    kernel.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = wintypes.HRSRC
    kernel.SizeofResource.argtypes = [wintypes.HMODULE, wintypes.HRSRC]
    kernel.SizeofResource.restype = wintypes.DWORD
    kernel.LoadResource.argtypes = [wintypes.HMODULE, wintypes.HRSRC]
    kernel.LoadResource.restype = wintypes.HGLOBAL
    kernel.LockResource.argtypes = [wintypes.HGLOBAL]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [wintypes.HMODULE]
    kernel.FreeLibrary.restype = wintypes.BOOL
    # AS_DATAFILE | AS_IMAGE_RESOURCE maps bytes only; no entry point or DLL
    # dependency code is executed while inspecting the PE resources.
    module = kernel.LoadLibraryExW(str(path), None, 0x02 | 0x20)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())

    def resource(kind, identifier):
        name = ctypes.c_wchar_p(identifier) if isinstance(identifier, str) else None
        key = ctypes.cast(name, ctypes.c_void_p) if name is not None else identifier
        handle = kernel.FindResourceW(module, key, kind)
        require(bool(handle), f"{path.name}: missing resource type {kind}, ID {identifier}")
        size = kernel.SizeofResource(module, handle)
        loaded = kernel.LoadResource(module, handle)
        address = kernel.LockResource(loaded)
        require(size > 0 and bool(address), f"{path.name}: could not read icon resource")
        return ctypes.string_at(address, size)

    try:
        group = resource(14, group_id)  # RT_GROUP_ICON; Inno names it MAINICON.
        require(len(group) >= 6, f"{path.name}: truncated icon group")
        reserved, kind, count = struct.unpack_from("<HHH", group)
        require((reserved, kind, count) == (0, 1, len(frames)), f"{path.name}: wrong icon frame count")
        require(len(group) == 6 + 14 * count, f"{path.name}: invalid icon group size")
        for index, (expected, payload) in enumerate(frames):
            entry = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * index)
            # RC resolves an ICO's unspecified PNG plane count (0) to 1;
            # Inno preserves 0. The actual RGBA PNG bytes must stay identical.
            normalized = (*expected[:4], expected[4] or 1, expected[5])
            require(entry[:6] in (expected, normalized) and entry[6] == len(payload), f"{path.name}: changed icon frame {index}")
            require(resource(3, entry[7]) == payload, f"{path.name}: icon pixels differ from shared ICO")
    finally:
        kernel.FreeLibrary(module)
    return {"exe": str(path), "frames": count, "resourceGroupId": group_id}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=pathlib.Path, action="append", default=[])
    parser.add_argument("--setup", type=pathlib.Path, action="append", default=[])
    args = parser.parse_args()
    frames = read_ico(ICON)
    binaries = [verify_exe(path, frames) for path in args.exe]
    binaries += [verify_exe(path, frames, "MAINICON") for path in args.setup]
    print(json.dumps({"passed": True, "icon": str(ICON), "sizes": sorted(REQUIRED_SIZES), "binaries": binaries}))


if __name__ == "__main__":
    main()
