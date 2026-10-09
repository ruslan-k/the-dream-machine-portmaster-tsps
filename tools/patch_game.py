#!/usr/bin/env python3
"""Select the game's existing SharedObject save backend, without recompiling AS2."""
import argparse, hashlib, struct, zlib
from pathlib import Path
ORIGINAL_SHA256 = '8d26128a342b60ae457c0a7476821d11959818e1669308c258125ba4269aa938'
OFFSET = 0x576BC
BEFORE = bytes.fromhex('960400081705003c')

def patch(data):
    if hashlib.sha256(data).hexdigest() != ORIGINAL_SHA256:
        raise ValueError('Unsupported game payload. Expected recovered GOG 1.0.6 SWF.')
    if data[:3] != b'FWS' or struct.unpack_from('<I', data, 4)[0] != len(data):
        raise ValueError('Expected complete uncompressed SWF')
    if data[OFFSET:OFFSET+len(BEFORE)] != BEFORE:
        raise ValueError('Unexpected AS2 declaration at the version-gated offset')
    result = bytearray(data)
    # Push constant-pool run_PC_as_MAC, Push Boolean false, DefineLocal.
    # Retain all action lengths, jumps, assets, chapter-access checks and save schema.
    result[OFFSET+6] = 1
    return bytes(result)

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('input', type=Path); p.add_argument('output', type=Path)
    a = p.parse_args()
    if a.input.resolve() == a.output.resolve(): p.error('Never overwrite the original SWF')
    result = patch(a.input.read_bytes())
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_bytes(result)
    print('SHA256', hashlib.sha256(result).hexdigest())

if __name__ == '__main__': main()
