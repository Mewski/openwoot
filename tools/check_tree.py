#!/usr/bin/env python3
"""Check the rules of the openwoot tree and the registration lists of built firmware.

    check_tree.py [--root DIR]           check the tree
    check_tree.py --elf FILE [FILE...]   check and print the registration lists of built
                                         firmware; files given together (the LTO=0 and the
                                         LTO=1 build of one board) must have the same lists
    check_tree.py --list-rules           the rules, one line each

The tree is built from layers (src/, drivers/, hal/, boards/) in which no board, family, driver
or feature is special, and this program names none of them either. A board is a directory
boards/<board>/ with a board.mk, a family a directory hal/<family>/ with a hal.mk, a driver a
directory drivers/<driver>/ with a driver.mk, a feature a directory src/<module>/<feature>/ with
a feature.mk, a strategy slot a directory src/<module>/<slot>/ with a strategy.mk. The names a
board is sold under come from its board.json and the strings of its board.h; the devices of a
family from the DEVICE_DEFS of the boards.

A registration list (src/core/registry.h) lies in the image between the markers
registry_<list>_begin and registry_<list>_end; an entry is the object registry_<list>_<function>
and holds a number (order or command number) followed by the address of the function it
registers. The firmware takes the first entry in link order of several with the same number.

Exit status 0 if no rule is broken.
"""
import argparse
import fnmatch
import json
import posixpath
import re
import struct
import sys
from collections import namedtuple
from pathlib import Path

LAYERS = {'src', 'drivers', 'hal', 'boards', 'third_party', 'tools', 'docs'}

# What a family directory must contain besides one implementation per interface header.
FAMILY_FILES = ['hal.mk', 'sections.ld', 'wiring_template.h', 'startup.c', 'vectors.c',
                'system_clock.c']
# What a board directory contains; the key position files are added per layout variant.
BOARD_FILES = ['board.mk', 'board.h', 'wiring.h', 'board.json', 'linker.ld', 'README.md',
               'tables/adc_scan_tables.c', 'tables/adc_scan_tables.h',
               'tables/layout_tables.c', 'tables/layout_tables.h',
               'tables/rgb_tables.c', 'tables/rgb_tables.h',
               'tables/profile_tables.c', 'tables/profile_tables.h',
               'defaults/profile_defaults.c', 'layout/leds.csv']
KEY_POSITIONS = 'layout/key_positions_*.csv'
# What the inline headers of a family may include: the fixed-width types and the board's wiring.
INLINE_INCLUDES = {'<stdint.h>', '<stdbool.h>', '"wiring.h"'}
# The line of a family's sections.ld that keeps the registration lists (src/core/registry.h).
REGISTRY_LINE = 'KEEP(*(SORT(.registry.*)))'
# The directory of the application layer; nothing outside it includes its headers.
APP_DIR = 'src/app'
# Where firmware that every board shares lives.
SHARED_CODE = ('src', 'drivers', 'hal/include')

# Peripheral and system register areas of a Cortex-M microcontroller: peripherals, the system
# control space, the device signature and option bytes.
REGISTER_ADDRESS = re.compile(r'\b0x(?:[45][0-9A-Fa-f]{7}|E00[0-9A-Fa-f]{5}|1FFF[0-9A-Fa-f]{4})[uUlL]*\b')
FEATURE_DEFINE = re.compile(r'\bFEATURE_([A-Z][A-Z0-9_]*)\b')
INCLUDE = re.compile(r'^\s*#\s*include\s+(<[^>]+>|"[^"]+")', re.M)
CONDITIONAL = re.compile(r'^[ \t]*#[ \t]*(?:if|ifdef|ifndef|elif)\b((?:[^\n]*\\\n)*[^\n]*)', re.M)
IDENTIFIER = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
DEFINE = re.compile(r'^#define\s+([A-Za-z_][A-Za-z0-9_]*)[ \t]*(.*)$', re.M)
STRING_CONSTANT = re.compile(r'^#define\s+[A-Za-z_][A-Za-z0-9_]*\s+"([^"]*)"', re.M)
SECTION = re.compile(r'^/\* ---- (.+?) -+ \*/$', re.M)
FEATURE_SECTION = 'Feature: '
COMMENT = re.compile(r'/\*.*?\*/|//[^\n]*', re.S)
# A function declaration or definition at the start of a line: `type name(`.
FUNCTION = re.compile(r'^(?:static\s+inline\s+)?[A-Za-z_][A-Za-z0-9_ \t\*]*?\b([a-z][a-z0-9_]*)\s*\(([^;{]*?)\)\s*([;{])',
                      re.M)


# ---- the tree --------------------------------------------------------------------------------

class Tree:
    """The tree and what its directories name."""

    def __init__(self, root):
        self.root = root
        self.boards = self.dirs('boards', 'board.mk')
        self.families = self.dirs('hal', 'hal.mk')
        self.drivers = self.dirs('drivers', 'driver.mk')
        self.features = {entry.parent.name: str(entry.parent.relative_to(root))
                         for entry in sorted(root.glob('src/*/*/feature.mk'))}
        self.slots = sorted(str(entry.parent.relative_to(root))
                            for entry in root.glob('src/*/*/strategy.mk'))

    def read(self, path):
        return (self.root / path).read_text(encoding='utf-8', errors='replace')

    def exists(self, path):
        return (self.root / path).exists()

    def dirs(self, path, marker):
        """Names of the sub-directories of `path` that contain the file `marker`."""
        return sorted(entry.parent.name for entry in (self.root / path).glob('*/' + marker))

    def files(self, path):
        """All files below `path`, relative to it, without hidden files."""
        base = self.root / path
        return sorted(str(entry.relative_to(base)) for entry in base.rglob('*')
                      if entry.is_file() and not entry.name.startswith('.'))

    def all_files(self, *paths):
        """All files below the given paths, relative to the root, without hidden files."""
        return [f'{path}/{name}' for path in paths for name in self.files(path)]

    def sources(self, *paths):
        """The .c and .h files below the given paths, relative to the root."""
        return [path for path in self.all_files(*paths) if path.endswith(('.c', '.h'))]


def make_variable(text, name):
    """The words of a `NAME := value` assignment in a make file, or []."""
    match = re.search(r'^' + re.escape(name) + r'\s*:?=\s*(.*)$', text, re.M)
    return match.group(1).split() if match else []


def code(text):
    """Source text without comments."""
    return COMMENT.sub(lambda comment: '\n' * comment.group(0).count('\n'), text)


def number(text):
    """The value of an integer constant as board.h writes it (0x08008000u, 2047), or None."""
    match = re.fullmatch(r'\(?\s*(0[xX][0-9A-Fa-f]+|\d+)[uUlL]*\s*\)?', text.split('/*')[0].strip())
    return int(match.group(1), 0) if match else None


def board_family(tree, board):
    family = make_variable(tree.read(f'boards/{board}/board.mk'), 'FAMILY')
    return family[0] if len(family) == 1 else None


def display_names(tree):
    """The words of the names the boards are sold under (the "name" of board.json and the
    strings of board.h), apart from the words that every board's names share (the maker)."""
    words = {}
    for board in tree.boards:
        texts = STRING_CONSTANT.findall(tree.read(f'boards/{board}/board.h')) \
            if tree.exists(f'boards/{board}/board.h') else []
        try:
            texts.append(json.loads(tree.read(f'boards/{board}/board.json'))['name'])
        except (OSError, ValueError, KeyError, TypeError):
            pass                    # reported by board-json
        words[board] = {word for text in texts for word in re.findall(r'[A-Za-z0-9]+', text)
                        if len(word) > 1}
    shared = set.intersection(*words.values()) if len(words) > 1 else set()
    return sorted(set().union(*words.values()) - shared) if words else []


def identity_names(tree):
    """Regular expressions for the names of the boards, families and devices: board directory
    names, family names (a trailing x of a family name stands for any character, as in the
    family names of the chip makers), the device defines of DEVICE_DEFS, the display names."""
    patterns = [re.escape(board) for board in tree.boards]
    for family in tree.families:
        stem = family.rstrip('x')
        patterns.append(re.escape(stem) + '[0-9a-z]' * (len(family) - len(stem)))
    for board in tree.boards:
        for define in make_variable(tree.read(f'boards/{board}/board.mk'), 'DEVICE_DEFS'):
            patterns.append(re.escape(define[2:].split('=')[0]) if define.startswith('-D') else '')
    patterns += [re.escape(name) for name in display_names(tree)]
    return sorted(set(pattern for pattern in patterns if pattern))


# ---- rules of the tree: each yields (path, message) --------------------------------------------

def foreign_directories(tree):
    """Top-level directories that are not layers."""
    return sorted(entry.name for entry in tree.root.iterdir()
                  if entry.is_dir() and not entry.name.startswith('.') and entry.name != 'build'
                  and entry.name not in LAYERS)


def rule_layers(tree):
    for name in foreign_directories(tree):
        yield name + '/', 'not a layer of the tree'
    for layer in sorted(LAYERS - {'third_party'}):
        for placeholder in sorted((tree.root / layer).rglob('.gitkeep')):
            yield str(placeholder.relative_to(tree.root)), 'placeholder: the directory has no content yet'


def rule_hal_files(tree):
    listing = {family: set(tree.files('hal/' + family)) for family in tree.families}
    everything = set().union(*listing.values()) if listing else set()
    for family in tree.families:
        for name in FAMILY_FILES:
            if name not in listing[family]:
                yield f'hal/{family}/{name}', 'missing'
        for name in sorted(everything - listing[family] - set(FAMILY_FILES)):
            yield f'hal/{family}/{name}', 'missing: another family has a file of this name'


def rule_hal_area(tree):
    areas = sorted(header.stem for header in (tree.root / 'hal/include/hal').glob('*.h'))
    for family in tree.families:
        listing = set(tree.files('hal/' + family))
        for area in areas:
            if not {area + '.c', f'inline/{area}_inline.h'} & listing:
                yield f'hal/{family}/{area}.c', f'missing: no implementation of hal/include/hal/{area}.h'


def rule_hal_inline(tree):
    for family in tree.families:
        for name in tree.files(f'hal/{family}'):
            if name.startswith('inline/'):
                path = f'hal/{family}/{name}'
                for include in INCLUDE.findall(tree.read(path)):
                    if include not in INLINE_INCLUDES:
                        yield path, f'includes {include}'


def functions(tree, paths, terminator):
    """Names of the functions that the files declare (';') or define ('{')."""
    return {match.group(1) for path in paths for match in FUNCTION.finditer(code(tree.read(path)))
            if match.group(3) == terminator}


def rule_hal_functions(tree):
    declared = {}
    for header in tree.sources('hal/include/hal'):
        for name in functions(tree, [header], ';'):
            declared[name] = header
    # A function of a HAL header that the firmware defines is a call-back: the family calls it.
    callbacks = functions(tree, tree.sources('src'), '{') & set(declared)
    for family in tree.families:
        defined = functions(tree, tree.sources('hal/' + family), '{')
        for name in sorted(set(declared) - callbacks - defined):
            yield f'hal/{family}/{name}', f'{name} of {declared[name]} is not defined'
        for name in sorted(callbacks & defined):
            yield f'hal/{family}/{name}', f'{name} is a call-back defined in src/ and is defined here as well'


def rule_hal_registry(tree):
    for family in tree.families:
        path = f'hal/{family}/sections.ld'
        if tree.exists(path) and REGISTRY_LINE not in re.sub(r'\s+', '', code(tree.read(path))):
            yield path, f'does not contain the line {REGISTRY_LINE} (src/core/registry.h)'


def rule_board_files(tree):
    for board in tree.boards:
        listing = set(tree.files('boards/' + board))
        expected = set(BOARD_FILES) | set(fnmatch.filter(listing, KEY_POSITIONS))
        for name in sorted(expected - listing):
            yield f'boards/{board}/{name}', 'missing'
        for name in sorted(listing - expected):
            yield f'boards/{board}/{name}', 'not part of the file set of a board directory'


def board_sections(text):
    """[(section title, [constant names])] of a board.h, in file order."""
    sections = [('', [])]
    for match in re.finditer(SECTION.pattern + '|' + DEFINE.pattern, text, re.M):
        if match.group(1) is not None:
            sections.append((match.group(1), []))
        else:
            sections[-1][1].append(match.group(2))
    return sections


def board_constants(text):
    """{name: value text} of the #define lines of a header."""
    return {name: value for name, value in DEFINE.findall(text)}


def rule_board_constants(tree):
    common = {}
    for board in tree.boards:
        path = f'boards/{board}/board.h'
        if not tree.exists(path):
            continue                # reported by board-files
        listed = make_variable(tree.read(f'boards/{board}/board.mk'), 'FEATURES')
        names = []
        for title, constants in board_sections(tree.read(path)):
            if not title.startswith(FEATURE_SECTION):
                names += constants
                continue
            feature = title[len(FEATURE_SECTION):]
            if feature not in tree.features:
                yield f'{path}:{feature}', f'block of the feature {feature}, which does not exist'
            elif feature not in listed:
                yield f'{path}:{feature}', f'block of the feature {feature}, which board.mk does not list'
        common[board] = names
    if not common:
        return
    first = next(iter(common))
    shared = [name for name in common[first] if all(name in names for names in common.values())]
    for board, names in common.items():
        for name in names:
            if name not in shared:
                yield f'boards/{board}/board.h:{name}', 'not defined by every board (and not in a feature block)'
        if [name for name in names if name in shared] != shared:
            yield f'boards/{board}/board.h', f'the constants are not in the order of boards/{first}/board.h'


def rule_board_wiring(tree):
    for board in tree.boards:
        family = board_family(tree, board)
        path = f'boards/{board}/wiring.h'
        template = f'hal/{family}/wiring_template.h'
        if family is None or not tree.exists(template) or not tree.exists(path):
            continue                # reported by board-files and by the build
        wanted = [name for name, _ in DEFINE.findall(tree.read(template))]
        found = [name for name, _ in DEFINE.findall(tree.read(path))]
        for name in wanted:
            if name not in found:
                yield f'{path}:{name}', f'missing: {template} lists it'
        for name in found:
            if name not in wanted:
                yield f'{path}:{name}', f'not listed by {template}'
        if sorted(wanted) == sorted(found) and wanted != found:
            yield path, f'the constants are not in the order of {template}'


def rule_board_json(tree):
    for board in tree.boards:
        base = f'boards/{board}/'
        if not all(tree.exists(base + name) for name in ('board.json', 'board.h', 'linker.ld')):
            continue                # reported by board-files
        try:
            description = json.loads(tree.read(base + 'board.json'))
            usb, image = description['usb'], description['image']
            told = {'BOARD_USB_VID': int(usb['vendor_id'], 16),
                    'BOARD_USB_PID_BASE': int(usb['product_id_base'], 16),
                    'BOARD_APP_BASE': int(image['base'], 16),
                    'BOARD_FLASH_END': int(image['end'], 16),
                    'BOARD_RAM_END': int(image['stack_top'], 16)}
            vectors = int(image['vectors'])
            description['name'], description['update']['supported']
        except (ValueError, KeyError, TypeError) as error:
            yield base + 'board.json', f'cannot be used: {error!r}'
            continue
        constants = board_constants(tree.read(base + 'board.h'))
        for name, value in told.items():
            if number(constants.get(name, '')) != value:
                yield base + 'board.json', f'{value:#010x} differs from {name} of board.h'

        regions = {name: (int(origin, 16), int(length, 16)) for name, origin, length in re.findall(
            r'^\s*(\w+)\s*\([rwx]+\)\s*:\s*ORIGIN\s*=\s*(0x[0-9A-Fa-f]+)\s*,\s*LENGTH\s*=\s*(0x[0-9A-Fa-f]+)',
            tree.read(base + 'linker.ld'), re.M)}
        flash, ram = regions.get('FLASH', (0, 0)), regions.get('RAM', (0, 0))
        if (flash[0], sum(flash), sum(ram)) != (told['BOARD_APP_BASE'], told['BOARD_FLASH_END'],
                                                 told['BOARD_RAM_END']):
            yield base + 'linker.ld', 'MEMORY differs from BOARD_APP_BASE, BOARD_FLASH_END, BOARD_RAM_END'

        table = f'hal/{board_family(tree, board)}/vectors.c'
        if tree.exists(table):
            size = re.search(r'\bg_vector_table\[(\d+)\]', tree.read(table))
            if size is None or int(size.group(1)) != vectors:
                yield base + 'board.json', f'"vectors": {vectors} differs from the table of {table}'


def rule_board_layout(tree):
    for board in tree.boards:
        if not tree.exists(f'boards/{board}/board.h'):
            continue                # reported by board-files
        constants = board_constants(tree.read(f'boards/{board}/board.h'))
        variants = number(constants.get('BOARD_LAYOUT_VARIANT_COUNT', ''))
        files = fnmatch.filter(tree.files('boards/' + board), KEY_POSITIONS)
        if variants != len(files):
            yield f'boards/{board}/layout', f'{len(files)} key position files, BOARD_LAYOUT_VARIANT_COUNT is {variants}'


def rule_driver_class(tree):
    for driver in tree.drivers:
        path = f'drivers/{driver}/driver.mk'
        klass = make_variable(tree.read(path), 'DRIVER_CLASS')
        if len(klass) != 1:
            yield path, 'does not set DRIVER_CLASS to one class'
        elif not tree.exists(f'drivers/include/drivers/{klass[0]}.h'):
            yield path, f'no header drivers/include/drivers/{klass[0]}.h for its class'


def rule_strategy_variants(tree):
    for slot in tree.slots:
        variants = sorted(entry.name for entry in (tree.root / slot).iterdir() if entry.is_dir())
        listing = {variant: set(tree.files(f'{slot}/{variant}')) for variant in variants}
        everything = set().union(*listing.values()) if listing else set()
        for variant in variants:
            for name in sorted(everything - listing[variant]):
                yield f'{slot}/{variant}/{name}', 'missing: another variant of the slot has a file of this name'


def library_headers(tree):
    """File names of the headers of every family's peripheral library."""
    names = set()
    for family in tree.families:
        for directory in make_variable(tree.read(f'hal/{family}/hal.mk'), 'VENDOR_DIR'):
            names |= {header.name for header in (tree.root / directory).rglob('*.h')}
    return names


def include_origin(tree, include, library, foreign):
    """Why shared code must not include this header, or None."""
    name = include.strip('<">')
    if name.split('/')[-1] in library:
        return 'a peripheral library'
    if name.split('/')[-1] == 'wiring.h':
        return "a board's wiring"
    for family in tree.families:
        if tree.exists(f'hal/{family}/{name}'):
            return f'hal/{family}/'
    for directory in foreign:
        if any((tree.root / directory).glob('*/' + name)):
            return directory + '/'
    return None


def rule_layer_include(tree):
    library = library_headers(tree)
    foreign = foreign_directories(tree)
    for path in tree.sources(*SHARED_CODE):
        for include in sorted(set(INCLUDE.findall(tree.read(path)))):
            origin = include_origin(tree, include, library, foreign)
            if origin is not None:
                yield path, f'includes {include} of {origin}'


def rule_app_include(tree):
    # An include is looked up next to the including file and below src/ (the include path of
    # the firmware); a path that leads into the application layer counts whether or not the
    # header is there.
    for path in tree.sources('src', 'drivers', 'hal', 'boards'):
        if path.startswith(APP_DIR + '/'):
            continue
        for include in sorted(set(INCLUDE.findall(tree.read(path)))):
            name = include.strip('<">')
            candidates = posixpath.normpath(posixpath.join(posixpath.dirname(path), name)), \
                posixpath.normpath('src/' + name)
            if any(candidate.startswith(APP_DIR + '/') for candidate in candidates):
                yield path, f'includes {include} of {APP_DIR}/'


def rule_register(tree):
    for path in tree.sources(*SHARED_CODE):
        addresses = sorted(set(match.group(0) for match in REGISTER_ADDRESS.finditer(code(tree.read(path)))))
        if addresses:
            yield path, 'register address ' + ', '.join(addresses)


def rule_feature_define(tree):
    public = {f'{directory}/{name}.h': name for name, directory in tree.features.items()}
    for path in tree.sources('src', 'drivers', 'hal', 'boards'):
        used = set(FEATURE_DEFINE.findall(tree.read(path)))
        allowed = {public[path].upper()} if path in public else set()
        for name in sorted(used - allowed):
            yield path, f'uses FEATURE_{name}; only the public header of the feature may'


def rule_identity_conditional(tree):
    pattern = re.compile('|'.join(identity_names(tree)), re.I)
    for path in tree.sources(*SHARED_CODE):
        for condition in CONDITIONAL.findall(code(tree.read(path))):
            found = sorted(set(word for word in IDENTIFIER.findall(condition) if pattern.search(word)))
            if found:
                yield path, 'conditional on ' + ', '.join(found)


def rule_names(tree):
    names = tree.boards + tree.families
    if not names:
        return
    pattern = re.compile(r'(?<![A-Za-z0-9])(' + '|'.join(map(re.escape, names)) + r')(?![A-Za-z0-9])')
    places = ['Makefile', 'tools/openwoot.py'] + tree.sources(*SHARED_CODE) + tree.all_files('.github')
    for path in places:
        if tree.exists(path):
            found = sorted(set(pattern.findall(tree.read(path))))
            if found:
                yield path, 'names ' + ', '.join(found)


def rule_display_names(tree):
    names = display_names(tree)
    if not names:
        return
    pattern = re.compile(r'(?<![A-Za-z0-9])(' + '|'.join(map(re.escape, names)) + r')(?![A-Za-z0-9])', re.I)
    for path in tree.all_files(*SHARED_CODE):
        found = sorted(set(pattern.findall(tree.read(path))))
        if found:
            yield path, 'names ' + ', '.join(found)


# ---- registration lists of built firmware ------------------------------------------------------

class Elf:
    """The symbols and the loaded contents of a 32-bit little-endian ELF file."""

    SHT_PROGBITS, SHT_SYMTAB, SHF_ALLOC, STT_FUNC = 1, 2, 2, 2

    def __init__(self, path):
        data = Path(path).read_bytes()
        if data[:4] != b'\x7fELF' or data[4:6] != b'\x01\x01':
            raise ValueError(f'{path}: not a 32-bit little-endian ELF file')
        shoff, = struct.unpack_from('<I', data, 0x20)
        shentsize, shnum = struct.unpack_from('<HH', data, 0x2E)
        # (name, type, flags, address, offset, size, link, info, align, entry size)
        self.sections = [struct.unpack_from('<10I', data, shoff + index * shentsize)
                         for index in range(shnum)]
        self.data = data
        self.symbols = []           # (name, value, size, type)
        for section in self.sections:
            if section[1] == self.SHT_SYMTAB:
                strings = self.sections[section[6]][4]
                for offset in range(section[4], section[4] + section[5], 16):
                    name, value, size, info, _, _ = struct.unpack_from('<IIIBBH', data, offset)
                    end = data.index(b'\0', strings + name)
                    self.symbols.append((data[strings + name:end].decode(), value, size, info & 0xF))

    def read(self, address, size):
        for _, kind, flags, start, offset, length, *_ in self.sections:
            if kind == self.SHT_PROGBITS and flags & self.SHF_ALLOC and start <= address \
                    and address + size <= start + length:
                return self.data[offset + address - start:offset + address - start + size]
        raise ValueError(f'{address:#010x} is not in a loaded section')


def plain(symbol):
    """A symbol name without the suffix the compiler adds to a local symbol (.lto_priv.0)."""
    return symbol.split('.')[0]


def registration_lists(elf):
    """{list: [(number, function)] in link order}, or a ValueError that says why not."""
    markers = {}
    for name, value, size, _ in elf.symbols:
        match = re.fullmatch(r'registry_(\w+)_(begin|end)', plain(name))
        if match and size == 0:
            if markers.setdefault(match.group(1), {}).setdefault(match.group(2), value) != value:
                raise ValueError(f'list {match.group(1)}: markers at two addresses')
    functions = {}
    for name, value, _, kind in elf.symbols:
        if kind == Elf.STT_FUNC:
            functions.setdefault(value & ~1, set()).add(plain(name))

    lists = {}
    for name, bounds in sorted(markers.items()):
        if set(bounds) != {'begin', 'end'}:
            raise ValueError(f'list {name}: a marker is missing')
        begin, end = bounds['begin'], bounds['end']
        prefix = f'registry_{name}_'
        entries = sorted({(value, size, plain(symbol)[len(prefix):]) for symbol, value, size, _ in elf.symbols
                          if plain(symbol).startswith(prefix) and size > 0 and begin <= value < end})
        lists[name] = []
        address = begin
        for value, size, entry in entries:
            if value != address:
                raise ValueError(f'list {name}: no entry symbol at {address:#010x}')
            record = elf.read(value, size)
            pointer = int.from_bytes(record[-4:], 'little') & ~1
            names = functions.get(pointer, set())
            function = entry if entry in names else min(names, default=f'{pointer:#010x}')
            lists[name].append((int.from_bytes(record[:-4], 'little'), function))
            address = value + size
        if address != end:
            raise ValueError(f'list {name}: no entry symbol at {address:#010x}')
    return lists


def in_order(entries):
    """The entries in the order of their numbers; entries with the same number in link order."""
    return sorted(entries, key=lambda entry: entry[0])


def rule_registry_found(builds):
    for path, lists in builds:
        if not lists:
            yield path, 'no registration list'
        for name, entries in lists.items():
            if not entries:
                yield path, f'{name}: the list is empty'


def rule_registry_unique(builds):
    for path, lists in builds:
        for name, entries in lists.items():
            first = {}
            for number_, function in entries:
                if number_ in first:
                    yield path, (f'{name}: {number_} is registered by {first[number_]} and by {function}; '
                                 f'the firmware takes {first[number_]}')
                else:
                    first[number_] = function


def rule_registry_same(builds):
    (reference, lists), others = builds[0], builds[1:]
    for path, other in others:
        for name in sorted(set(lists) | set(other)):
            mine, theirs = in_order(lists.get(name, [])), in_order(other.get(name, []))
            if mine != theirs:
                only_mine = [entry for entry in mine if entry not in theirs]
                only_theirs = [entry for entry in theirs if entry not in mine]
                detail = '; '.join(f'only in {where}: ' + ', '.join(f'{n} {f}' for n, f in entries)
                                   for where, entries in ((reference, only_mine), (path, only_theirs)) if entries)
                yield path, f'{name}: differs from {reference}' + (f' ({detail})' if detail else ' (order)')


def print_lists(path, lists):
    for name, entries in lists.items():
        print(f'{path}: {name}, {len(entries)} entries')
        for number_, function in in_order(entries):
            print(f'  {number_:5}  {function}')


# ---- the rules -------------------------------------------------------------------------------

Rule = namedtuple('Rule', 'name description check')

TREE_RULES = [
    Rule('layers', 'the top level holds the layer directories and nothing else; no placeholder files', rule_layers),
    Rule('hal-files', 'every hal/<family>/ has the same file names, the family files included', rule_hal_files),
    Rule('hal-area', 'every header of hal/include/hal/ has its .c or inline header in every family', rule_hal_area),
    Rule('hal-functions', 'every function of a HAL header is defined by every family (call-backs excepted)', rule_hal_functions),
    Rule('hal-inline', "the inline headers of a family include only <stdint.h>, <stdbool.h> and the board's wiring.h", rule_hal_inline),
    Rule('hal-registry', f'every hal/<family>/sections.ld contains {REGISTRY_LINE}', rule_hal_registry),
    Rule('board-files', 'every boards/<board>/ has the same, complete file set', rule_board_files),
    Rule('board-constants', 'every board.h defines the same constants in the same order; feature blocks only for listed features', rule_board_constants),
    Rule('board-wiring', "every wiring.h defines the constants of its family's wiring_template.h, in order", rule_board_wiring),
    Rule('board-json', "board.json agrees with board.h, linker.ld and the family's vector table", rule_board_json),
    Rule('board-layout', 'one key_positions_<variant>.csv per layout variant', rule_board_layout),
    Rule('driver-class', 'every driver names a class that has a header in drivers/include/drivers/', rule_driver_class),
    Rule('strategy-variants', 'every variant directory of a strategy slot has the same file names', rule_strategy_variants),
    Rule('layer-include', 'src/, drivers/, hal/include/ include no header of a peripheral library, a family directory '
                          "(apart from its inline headers), a board's wiring or a directory outside the layers", rule_layer_include),
    Rule('app-include', f'no file outside {APP_DIR}/ includes a header of {APP_DIR}/', rule_app_include),
    Rule('register', 'src/, drivers/, hal/include/ contain no address of a peripheral register', rule_register),
    Rule('feature-define', 'FEATURE_<NAME> occurs only in the public header of that feature', rule_feature_define),
    Rule('identity-conditional', 'no preprocessor conditional in src/, drivers/, hal/include/ on a board, family or device name', rule_identity_conditional),
    Rule('names', 'no board or family directory name in the Makefile, src/, drivers/, hal/include/, tools/openwoot.py, .github/', rule_names),
    Rule('display-names', 'no name a board is sold under (board.json, strings of board.h) in src/, drivers/, hal/include/', rule_display_names),
]

ELF_RULES = [
    Rule('registry-found', 'every built image has registration lists, and none of them is empty', rule_registry_found),
    Rule('registry-unique', 'no two entries of one list have the same order or command number', rule_registry_unique),
    Rule('registry-same', 'the images given together (LTO=0, LTO=1) have the same lists', rule_registry_same),
]


def report(rules, subject):
    failures = [f'{rule.name}: {path}: {message}' for rule in rules for path, message in rule.check(subject)]
    for failure in failures:
        print(failure)
    return failures


def check_tree(root):
    tree = Tree(root)
    failures = report(TREE_RULES, tree)
    print(f'{len(tree.boards)} boards ({", ".join(tree.boards)}), {len(tree.families)} families '
          f'({", ".join(tree.families)}), {len(tree.drivers)} drivers, {len(tree.features)} features, '
          f'{len(tree.slots)} strategy slots')
    print(f'failures: {len(failures)}' if failures else 'the tree follows every rule')
    return 1 if failures else 0


def check_elf(paths):
    builds = []
    for path in paths:
        try:
            builds.append((path, registration_lists(Elf(path))))
        except (OSError, ValueError) as error:
            print(f'registry-found: {path}: {error}')
            return 1
    print_lists(*builds[0])
    failures = report(ELF_RULES, builds)
    print(f'failures: {len(failures)}' if failures else
          f'the registration lists are complete, unique and the same in {len(builds)} image(s)')
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description='Check the rules of the openwoot tree and the registration '
                                                 'lists of built firmware.')
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent.parent,
                        help='the tree to check (default: the one this file is in)')
    parser.add_argument('--elf', nargs='+', metavar='FILE',
                        help='check the registration lists of these ELF files (the builds of one board) '
                             'instead of the tree')
    parser.add_argument('--list-rules', action='store_true', help='print the rules and exit')
    arguments = parser.parse_args()
    if arguments.list_rules:
        for title, rules in (('tree', TREE_RULES), ('--elf', ELF_RULES)):
            for rule in rules:
                print(f'{title:6} {rule.name:21} {rule.description}')
        return
    sys.exit(check_elf(arguments.elf) if arguments.elf else check_tree(arguments.root.resolve()))


if __name__ == '__main__':
    main()
