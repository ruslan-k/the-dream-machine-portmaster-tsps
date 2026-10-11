#!/usr/bin/env python3
"""Check every room SWF's ImportAssets/2 names against the source assets' exports.

Finds the "dighole pit" class of bug statically: an import whose export name is
not present in the named source file can never be registered, so the room would
be missing that character even with a correct import implementation.
"""
import struct
import zlib
import sys
import glob
import os


def tags(data):
    sig = data[:3]
    if sig == b'FWS':
        body = data[8:]
    elif sig == b'CWS':
        body = zlib.decompress(data[8:])
    else:
        raise ValueError('unsupported signature %r' % sig)
    # Skip the frame header: RECT (5-bit nbits + 4*nbits values, byte-aligned)
    # followed by frame rate (u16) and frame count (u16).
    nbits = body[0] >> 3
    pos = (5 + 4 * nbits + 7) // 8 + 4
    n = len(body)
    while pos + 2 <= n:
        v = struct.unpack_from('<H', body, pos)[0]
        pos += 2
        code = v >> 6
        length = v & 0x3F
        if length == 0x3F:
            if pos + 4 > n:
                break
            length = struct.unpack_from('<I', body, pos)[0]
            pos += 4
        if pos + length > n:
            break
        chunk = body[pos:pos + length]
        pos += length
        if code == 0:
            break
        yield code, chunk


def cstr(b, pos):
    end = b.index(b'\x00', pos)
    return b[pos:end].decode('utf-8', 'replace'), end + 1


def exports(data):
    names = []
    for code, body in tags(data):
        if code == 56:  # ExportAssets
            pos = 0
            count = struct.unpack_from('<H', body, pos)[0]
            pos += 2
            for _ in range(count):
                pos += 2
                name, pos = cstr(body, pos)
                names.append(name)
    return names


def imports(data):
    out = []
    for code, body in tags(data):
        try:
            if code == 71:  # ImportAssets2
                pos = 0
                url, pos = cstr(body, pos)
                pos += 2
                count = struct.unpack_from('<H', body, pos)[0]
                pos += 2
                for _ in range(count):
                    pos += 2
                    name, pos = cstr(body, pos)
                    out.append((url, name))
            elif code == 57:  # ImportAssets (v1)
                pos = 0
                url, pos = cstr(body, pos)
                count = struct.unpack_from('<H', body, pos)[0]
                pos += 2
                for _ in range(count):
                    pos += 2
                    name, pos = cstr(body, pos)
                    out.append((url, name))
        except (IndexError, struct.error, ValueError):
            out.append(('<?parse-error>', ''))
    return out


def main():
    root = sys.argv[1]
    sources = {}
    for path in glob.glob(os.path.join(root, '*.swf')):
        try:
            sources[os.path.basename(path)] = set(exports(open(path, 'rb').read()))
        except Exception as e:
            print('source %s: parse failed: %s' % (path, e))
    print('sources: %s' % ', '.join('%s(%d exports)' % (k, len(v)) for k, v in sorted(sources.items())))

    problems = []
    rooms = 0
    total_imports = 0
    for ch in sorted(glob.glob(os.path.join(root, 'TDM_chapter*'))):
        for path in sorted(glob.glob(os.path.join(ch, '*.swf'))):
            rooms += 1
            try:
                data = open(path, 'rb').read()
                for url, name in imports(data):
                    total_imports += 1
                    base = os.path.basename(url)
                    exp = sources.get(base)
                    if exp is None:
                        problems.append((path, url, name, 'SOURCE-MISSING'))
                    elif name and name not in exp:
                        problems.append((path, url, name, 'NAME-NOT-EXPORTED'))
            except Exception as e:
                problems.append((path, '', str(e), 'PARSE-FAILED'))

    print('rooms checked: %d, import entries: %d' % (rooms, total_imports))
    print('problems: %d' % len(problems))
    for path, url, name, kind in problems:
        print('%s: %s imports %r from %s' % (kind, os.path.basename(path), name, url))


if __name__ == '__main__':
    main()
