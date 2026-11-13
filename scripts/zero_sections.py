# The sections of an ELF64 relocatable object (little endian) that are in
# memory (SHF_ALLOC), have the size 0, and that a relocation of a section
# in memory names: one name on each line.
#
# Usage: python3 scripts/zero_sections.py OBJECT
#
# A static of Rust with a type of size 0 (for example the logger NOP of
# the crate log) has a section of size 0, and code takes its address. The
# loader of the native NIF objects of beam.com puts no section of size 0
# into memory, so such an object does not load ("the symbol is in a
# section that is not loaded"). scripts/nif.sh gives each of these
# sections one byte.
import struct
import sys

data = open(sys.argv[1], 'rb').read()
if data[:4] != b'\x7fELF' or data[4] != 2 or data[5] != 1:
    sys.exit('zero_sections.py: %s is not an ELF64 file (little endian)' % sys.argv[1])
SHT_SYMTAB, SHT_RELA, SHF_ALLOC = 2, 4, 2
shoff, = struct.unpack_from('<Q', data, 0x28)
shentsize, shnum, shstrndx = struct.unpack_from('<HHH', data, 0x3A)
# name, type, flags, addr, offset, size, link, info, addralign, entsize
sections = [struct.unpack_from('<IIQQQQIIQQ', data, shoff + i * shentsize) for i in range(shnum)]


def section_name(i):
    start = sections[shstrndx][4] + sections[i][0]
    return data[start:data.index(b'\0', start)].decode()


symtab = next(s for s in sections if s[1] == SHT_SYMTAB)
symbols = [struct.unpack_from('<IBBHQQ', data, symtab[4] + j * 24) for j in range(symtab[5] // 24)]
found = set()
for s in sections:
    if s[1] != SHT_RELA or not sections[s[7]][2] & SHF_ALLOC:
        continue
    for j in range(s[5] // 24):
        info, = struct.unpack_from('<Q', data, s[4] + j * 24 + 8)
        index = symbols[info >> 32][3]
        # Not SHN_UNDEF, and not a special index (SHN_ABS, SHN_COMMON).
        if 0 < index < 0xff00 and sections[index][2] & SHF_ALLOC and sections[index][5] == 0:
            found.add(index)
for index in sorted(found):
    print(section_name(index))
