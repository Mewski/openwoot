#!/usr/bin/env python3
"""Host tool for openwoot: device information, update mode and flashing.

Usage:

    openwoot.py [--log FILE] COMMAND [ARGUMENT ...]

    info             identification and state of the running firmware (reads only)
    raw CMD [ARG]    send one read-only feature command to the running firmware
    enter            ask the running firmware to reset into update mode
    bootinfo         update mode, reads only: flags, bootloader version, serial number, session CRC
    plan IMAGE       offline: block count and CRC of an image, sanity checks
    flash IMAGE      update mode: write the image, verify it, start it
    reboot [0|1]     leave update mode
    combo            check the recovery key combination while the keyboard is replugged

IMAGE is the Intel HEX file of a build (build/<board>/openwoot-<board>.hex) or the flat binary
next to it (.bin). `openwoot.py COMMAND --help` describes each command; tools/README.md shows a
complete session.

The tool has no list of keyboards of its own. Every board is described by its
boards/<board>/board.json: USB ids, image limits, and whether its update procedure is known. A
connected keyboard is recognised by its USB ids, an image by its start address and its initial
stack pointer; --board names the board where that is not enough (a flat binary has no address).
`enter` and `flash` refuse a board whose update procedure is not described.

The keyboard answers in one of two modes, told apart by the usage page of the HID interface:

  running firmware   usage page 0xFF55. A request is the feature report with report id 1,
                     `D1 DA command argument`; the reply is an input report
                     `D1 DA command status length payload` (docs/protocol.md).
  update mode        usage page 0xFFFF, the update product id of the board: the factory
                     bootloader. A request is a 7-byte feature report `DF DB command parameter`;
                     the reply is a 256-byte input report `DF DB command status length body`.
                     Image data travels as output reports, one per block (docs/bootloader.md).

The only dependency is the Python package `hidapi` (tools/requirements.txt).
"""
import argparse
import datetime
import json
import struct
import sys
import time
from pathlib import Path

try:
    import hid
except ImportError:
    hid = None

STATUS_OK = 0x88

# ---- running firmware: configuration interface (docs/protocol.md) ----------------------------

FW_USAGE_PAGE = 0xFF55
FW_REPORT_ID = b'\x01'
FW_MAGIC = b'\xD1\xDA'
FW_READ_SIZE = 2048              # largest input report of the interface, with room to spare
FW_REPLY_TIMEOUT_MS = 2000
FW_CMD_VERSION = 1
FW_CMD_ENTER_UPDATE_MODE = 2     # resets the keyboard into the bootloader; no reply
FW_STATUS_NAMES = {0x88: 'ok', 0xFF: 'error', 0x66: 'unknown command', 0x77: 'queue full',
                   0xAA: 'unsupported'}
USB_SPEED_NAMES = {1: 'low', 2: 'full', 3: 'high'}
READ_PROTECTION_OFF = 0x11       # how the firmware reports an unprotected microcontroller

# Feature commands that neither write to flash nor change the state of the keyboard.
# `raw` sends no other command.
READ_ONLY_COMMANDS = {
    0: 'ping', 1: 'version', 3: 'serial', 11: 'active profile', 19: 'device config',
    58: 'storage usage', 62: 'profile count', 69: 'usb speed', 73: 'active profile state',
    81: 'lock mode',
}

# ---- update mode: the factory bootloader (docs/bootloader.md) ---------------------------------

BOOT_USAGE_PAGE = 0xFFFF
BOOT_MAGIC = b'\xDF\xDB'
NO_REPORT_ID = b'\x00'           # the interface has no report ids; hidapi wants this byte in front
BOOT_READ_SIZE = 1024            # replies are 256 bytes
BOOT_REPLY_TIMEOUT_MS = 4000
BOOT_ECHO_ANY = 0xFF             # accepted in place of the echoed command number of a reply
BOOT_OPEN_WAIT_S = 3             # how long a command waits for the keyboard in update mode
MODE_CHANGE_WAIT_S = 20          # how long the keyboard may take to appear in the other mode
ENUMERATION_POLL_S = 0.5
REPLUG_POLL_S = 0.2
FIRMWARE_WAIT_S = 5              # `combo`: how long the firmware may take to appear after a replug

BOOT_CMD_PING = 0                # body: inverted low byte of the parameter, then the flag byte
BOOT_CMD_VERSION = 1             # body: major, minor
BOOT_CMD_START_UPLOAD = 2        # parameter: block count; erases the completion marker
BOOT_CMD_WRITE_BLOCK = 3         # parameter: block index; programs the block sent last
BOOT_CMD_CRC = 4                 # body: CRC-24 of the blocks written since the bootloader started
BOOT_CMD_REBOOT = 5              # parameter bit 0: write the completion marker; no reply
BOOT_CMD_UNLOCK = 6              # parameter UNLOCK_KEY: allows commands 2, 3 and 8
BOOT_CMD_LAST_BLOCK = 8          # body: index of the block written last
BOOT_CMD_SERIAL = 9              # body: serial number as text

UNLOCK_KEY = 0xFFAAFFBB          # erases nothing; holds until the next reset
REBOOT_PLAIN = 0                 # restart without touching flash
REBOOT_MARK_COMPLETE = 1         # write the completion marker, then restart
PING_ATTEMPTS = 5
PING_TIMEOUT_MS = 1000
PING_RETRY_S = 0.5

FLAG_KEY_COMBINATION = 0x01
FLAG_NAMES = {
    FLAG_KEY_COMBINATION: 'recovery key combination held',
    0x02: 'update mode requested by the firmware',
    0x04: 'no valid firmware installed',
}

# ---- boards and images -------------------------------------------------------------------------

BOARDS_DIR = Path(__file__).resolve().parent.parent / 'boards'
USB_MODE_COUNT = 3               # product id of running firmware: product_id_base + USB mode
VECTOR_SIZE = 4                  # bytes per entry of the vector table at the start of an image
CODE_ALIGNMENT = 64              # the first code byte follows the vector table on this boundary

LABEL_WIDTH = 14
NO_KEYBOARD = ('no keyboard found that a board description (boards/*/board.json) names; '
               'is it plugged in?')
STAYS_IN_UPDATE_MODE = 'The keyboard stays in update mode; run the flash command again.'

_log_file = None


class Failure(Exception):
    """An error that ends the program with a one-line message."""


class BootloaderError(Failure):
    """A request to the bootloader failed, or its answer is not the expected one."""


# ---- board descriptions (boards/<board>/board.json) --------------------------------------------

class Board:
    """One boards/<board>/board.json: what this tool needs to know about a keyboard."""

    def __init__(self, path):
        try:
            description = json.loads(path.read_text(encoding='utf-8'))
            usb, image, update = description['usb'], description['image'], description['update']
            self.key = path.parent.name               # the directory name: what --board takes
            self.name = description['name']
            self.vendor_id = int(usb['vendor_id'], 16)
            self.product_id_base = int(usb['product_id_base'], 16)
            self.app_base = int(image['base'], 16)    # start of an image, behind the bootloader
            self.app_end = int(image['end'], 16)      # an image ends below this address
            self.stack_top = int(image['stack_top'], 16)  # first word of an image: end of RAM
            vector_bytes = int(image['vectors']) * VECTOR_SIZE
            self.code_offset = -(-vector_bytes // CODE_ALIGNMENT) * CODE_ALIGNMENT
            self.update_supported = update['supported'] is True
            self.update_reason = update.get('reason', 'no reason given')
            self.update_product_id = None             # product id of the keyboard in update mode
            self.block_size = None                    # bytes per block of the update protocol
            if self.update_supported:
                self.update_product_id = int(update['product_id'], 16)
                self.block_size = int(update['block_size'])
        except (OSError, ValueError, KeyError, TypeError) as error:
            raise Failure(f'cannot use the board description {path}: {error!r}') from error

    def require_update_support(self):
        """End the program unless the update procedure of this board is described."""
        if not self.update_supported:
            raise Failure(f'update mode and flashing are not available for the {self.name}: '
                          f'{self.update_reason}; nothing was changed')


_boards = None


def load_boards():
    """All board descriptions, in the order of their directory names."""
    global _boards
    if _boards is None:
        _boards = [Board(path) for path in sorted(BOARDS_DIR.glob('*/board.json'))]
        if not _boards:
            raise Failure(f'no board description found ({BOARDS_DIR}/*/board.json)')
    return _boards


def board_names():
    return ', '.join(board.key for board in load_boards())


def board_by_name(name):
    for board in load_boards():
        if board.key == name:
            return board
    raise Failure(f"unknown board '{name}'; boards: {board_names()}")


# ---- output and log ----------------------------------------------------------------------------

def log(text):
    """Append one line with a timestamp to the log file, if --log was given."""
    if _log_file is not None:
        stamp = datetime.datetime.now().strftime('%H:%M:%S.%f')[:-3]
        _log_file.write(f'{stamp} {text}\n')
        _log_file.flush()


def say(text):
    print(text)
    log('# ' + text)


def show(label, text):
    say(f'{label:<{LABEL_WIDTH}}: {text}')


def open_log(path):
    global _log_file
    try:
        _log_file = open(path, 'a', encoding='utf-8')
    except OSError as error:
        raise Failure(f'cannot open the log file {path}: {error.strerror}') from error
    log(f'==== {datetime.date.today()} {" ".join(sys.argv[1:])}')


# ---- image handling ----------------------------------------------------------------------------

def parse_intel_hex(text):
    """Return (address of the first data record, data with the gaps filled with FF).

    The first data record defines offset 0, as in the update protocol: the bootloader is given
    block numbers, never addresses.
    """
    base = 0
    first = None
    data = bytearray()
    seen_eof = False
    for number, line in enumerate(text.split(), 1):
        if not line.startswith(':'):
            raise ValueError(f'line {number} does not start with a colon')
        raw = bytes.fromhex(line[1:])
        if len(raw) < 5 or len(raw) != raw[0] + 5:
            raise ValueError(f'line {number}: bad record length')
        if sum(raw) & 0xFF:
            raise ValueError(f'line {number}: bad record checksum')
        count, offset, kind, payload = raw[0], raw[1] << 8 | raw[2], raw[3], raw[4:-1]
        if kind == 0:                                   # data
            address = base + offset
            if first is None:
                first = address
            position = address - first
            if position < 0:
                raise ValueError(f'line {number}: address below the first data record')
            if position > len(data):
                data += b'\xFF' * (position - len(data))
            data[position:position + count] = payload
        elif kind == 1:                                 # end of file
            seen_eof = True
        elif kind == 2:                                 # extended segment address
            base = int.from_bytes(payload, 'big') << 4
        elif kind == 4:                                 # extended linear address
            base = int.from_bytes(payload, 'big') << 16
        elif kind not in (3, 5):                        # start addresses are not used
            raise ValueError(f'line {number}: record type {kind}')
    if not seen_eof:
        raise ValueError('no end-of-file record')
    if first is None:
        raise ValueError('no data record')
    return first, bytes(data)


def load_image(path):
    """Return (start address, content) of an image file.

    A file whose first character is a colon is read as Intel HEX, anything else as a flat binary,
    which has no start address (None).
    """
    try:
        content = Path(path).read_bytes()
    except OSError as error:
        raise Failure(f'cannot read {path}: {error.strerror}') from error
    if not content.lstrip().startswith(b':'):
        return None, content
    try:
        return parse_intel_hex(content.decode('ascii'))
    except ValueError as error:
        raise Failure(f'{path} is not a valid Intel HEX file: {error}') from error


def board_of_image(first, data, board_name):
    """The board an image is built for: the named one, or the one whose description it fits."""
    if len(data) < 2 * VECTOR_SIZE:
        raise Failure('the image is empty')
    if board_name is not None:
        board = board_by_name(board_name)
        if first not in (None, board.app_base):
            raise Failure(f'image starts at {first:#010x}, expected {board.app_base:#010x}')
        return board
    stack_pointer = struct.unpack_from('<I', data, 0)[0]
    fitting = [board for board in load_boards()
               if first in (None, board.app_base) and stack_pointer == board.stack_top]
    if len(fitting) == 1:
        return fitting[0]
    start = 'no start address' if first is None else f'start address {first:#010x}'
    if not fitting:
        raise Failure(f'the image fits no board description ({start}, initial stack pointer '
                      f'{stack_pointer:#010x}); boards: {board_names()}')
    raise Failure(f'the image fits several boards ({", ".join(board.key for board in fitting)}); '
                  'name one with --board')


def crc24(data):
    """CRC that bootloader command 4 reports: 24 bits, over little-endian 16-bit words.

    Test values: the bytes 01 02 03 04 give 0x000001, the 256 bytes 00..FF give 0xEF3F62.
    """
    crc = 0
    for i in range(0, len(data), 2):
        word = data[i] | data[i + 1] << 8
        feedback = 0x80001B if crc & 0x800000 else 0
        crc = (((crc << 1) & 0xFFFFFE) ^ word ^ feedback) & 0xFFFFFF
    return crc


def sanity_check_image(image, board):
    """Refuse anything that does not look like firmware for this board."""
    limit = board.app_end - board.app_base
    if len(image) > limit:
        raise Failure(f'image has {len(image)} bytes; the application area of the {board.name} has '
                      f'{limit} ({board.app_base:#010x}-{board.app_end - 1:#010x})')
    stack_pointer, reset = struct.unpack_from('<II', image, 0)
    if stack_pointer != board.stack_top:
        raise Failure(f'initial stack pointer {stack_pointer:#010x}, '
                      f'expected {board.stack_top:#010x}')
    if not (board.app_base + board.code_offset <= reset < board.app_base + len(image)
            and reset & 1):
        raise Failure(f'reset vector {reset:#010x} does not point into the image as Thumb code')


def plan_image(path, board_name):
    """Load and check an image and print what an update would send.

    Returns (board, image). For a board with a known update procedure the image is padded with
    FF to whole blocks.
    """
    first, image = load_image(path)
    board = board_of_image(first, image, board_name)
    if board.update_supported and len(image) % board.block_size:
        image += b'\xFF' * (board.block_size - len(image) % board.block_size)
    sanity_check_image(image, board)
    show('board', f'{board.name} (boards/{board.key})')
    if board.update_supported:
        show('image', f'{path}, {len(image)} bytes after padding')
        show('blocks', f'{len(image) // board.block_size} of {board.block_size} bytes')
        show('crc', f'{crc24(image):#08x}')
    else:
        show('image', f'{path}, {len(image)} bytes')
        show('update', f'not supported: {board.update_reason}')
    return board, image


# ---- finding and opening the keyboard ----------------------------------------------------------

def find_firmware():
    """HID interfaces of running firmware: a product id of a board description, the usage page."""
    vendor_ids = sorted({board.vendor_id for board in load_boards()})
    return [entry for vendor_id in vendor_ids for entry in hid.enumerate(vendor_id, 0)
            if entry['usage_page'] == FW_USAGE_PAGE and board_of_firmware(entry) is not None]


def find_bootloader():
    """HID interfaces of keyboards in update mode, for the boards whose update mode is known."""
    return [entry for board in load_boards() if board.update_supported
            for entry in hid.enumerate(board.vendor_id, board.update_product_id)
            if entry['usage_page'] == BOOT_USAGE_PAGE]


def board_of_firmware(entry):
    """The board whose running firmware has the USB ids of this interface, or None."""
    for board in load_boards():
        mode = entry['product_id'] - board.product_id_base
        if entry['vendor_id'] == board.vendor_id and 0 <= mode < USB_MODE_COUNT:
            return board
    return None


def board_of_bootloader(entry):
    """The board whose update mode has the USB ids of this interface."""
    return next(board for board in load_boards() if board.update_supported
                and (entry['vendor_id'], entry['product_id'])
                == (board.vendor_id, board.update_product_id))


def describe(entry):
    return f'{entry["vendor_id"]:04x}:{entry["product_id"]:04x} "{entry["product_string"]}"'


def wait_for(finder, seconds, what):
    """Poll the device list until `finder` returns an interface; return the first one."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        found = finder()
        if found:
            return found[0]
        time.sleep(ENUMERATION_POLL_S)
    raise Failure(f'{what} did not appear within {seconds} s')


def require_firmware():
    """Return the configuration interface of the running firmware or explain what is there."""
    found = find_firmware()
    if found:
        return found[0]
    if find_bootloader():
        raise Failure('the keyboard is in update mode, this command needs the running firmware; '
                      'flash an image, or leave update mode with "openwoot.py reboot"')
    raise Failure(NO_KEYBOARD)


def require_bootloader():
    """Return the interface of the keyboard in update mode or explain what is there."""
    try:
        return wait_for(find_bootloader, BOOT_OPEN_WAIT_S, 'the keyboard in update mode')
    except Failure:
        if find_firmware():
            raise Failure('the keyboard runs its firmware, this command needs update mode; '
                          'run "openwoot.py enter" first') from None
        raise Failure(NO_KEYBOARD) from None


def open_device(entry):
    device = hid.device()
    try:
        device.open_path(entry['path'])
    except OSError as error:
        if sys.platform.startswith('linux'):
            hint = ('install tools/udev/70-openwoot.rules (see tools/README.md) '
                    'and replug the keyboard')
        else:
            hint = 'close other programs that use the keyboard, such as Wootility'
        raise Failure(f'cannot open {describe(entry)}: {hint}') from error
    return device


def drain(device, size):
    """Drop input reports that are still waiting from earlier requests."""
    device.set_nonblocking(1)
    while device.read(size):
        pass
    device.set_nonblocking(0)


# ---- running firmware --------------------------------------------------------------------------

class Firmware:
    """The configuration interface of the running firmware."""

    def __init__(self):
        self.entry = require_firmware()
        self.board = board_of_firmware(self.entry)
        self.device = open_device(self.entry)

    def __enter__(self):
        return self

    def __exit__(self, *exc_info):
        self.device.close()

    def _send(self, command, argument):
        drain(self.device, FW_READ_SIZE)
        packet = FW_REPORT_ID + FW_MAGIC + bytes([command]) + struct.pack('<I', argument)
        log(f'-> firmware feature {packet.hex()}')
        self.device.send_feature_report(packet)

    def command(self, command, argument=0):
        """Send one feature command; return (status, payload), or None if no reply arrives."""
        self._send(command, argument)
        report = bytes(self.device.read(FW_READ_SIZE, FW_REPLY_TIMEOUT_MS))
        log(f'<- firmware input   {report[:16].hex()}')
        if not report:
            return None
        frame = report[1:]                              # the first byte is the report id
        if len(frame) < 6 or frame[:2] != FW_MAGIC or frame[2] != command:
            raise Failure(f'unexpected reply to feature command {command}: {report[:32].hex()}')
        length = frame[4] | frame[5] << 8
        return frame[3], frame[6:6 + length]

    def reset_into_update_mode(self):
        """Feature command 2. The keyboard resets at once, so the transfer may end with an error."""
        try:
            self._send(FW_CMD_ENTER_UPDATE_MODE, 0)
        except OSError as error:
            log(f'send ended with {error!r} (expected when the keyboard resets at once)')


def format_device_config(payload):
    option = payload[4]
    protection = 'off' if option == READ_PROTECTION_OFF else f'ON (reported as {option:#04x})'
    return f'{payload.hex()} (MCU read protection {protection})'


# One line of `info` each: label, feature command, argument, shortest usable payload, formatter.
INFO_QUERIES = (
    ('firmware', FW_CMD_VERSION, 0, 3, lambda p: f'{p[0]}.{p[1]}.{p[2]}'),
    ('usb speed', 69, 0, 1, lambda p: USB_SPEED_NAMES.get(p[0], 'unknown')),
    ('mcu unique id', 3, 1, 1, bytes.hex),
    ('spi flash', 3, 3, 3,
     lambda p: f'manufacturer {p[0]:#04x}, device {p[1]:#04x}, unique id {p[2:].hex()}'),
    ('device config', 19, 0, 5, format_device_config),
    ('active profile', 11, 0, 2, lambda p: f'index {p[0]}, namespace {p[1]}'),
    ('profiles', 62, 0, 1, lambda p: str(p[0])),
    ('lock mode', 81, 0, 1, lambda p: str(p[0])),
)


def cmd_info(args):
    with Firmware() as firmware:
        entry = firmware.entry
        show('device', f'{entry["product_string"]}, product id {entry["product_id"]:#06x}, '
                       f'serial {entry["serial_number"]}')
        for label, command, argument, shortest, formatter in INFO_QUERIES:
            reply = firmware.command(command, argument)
            if reply is not None and reply[0] == STATUS_OK and len(reply[1]) >= shortest:
                show(label, formatter(reply[1]))


def cmd_raw(args):
    if args.command not in READ_ONLY_COMMANDS:
        raise Failure(f'feature command {args.command} is not in the read-only list of this tool '
                      f'({", ".join(map(str, sorted(READ_ONLY_COMMANDS)))})')
    with Firmware() as firmware:
        reply = firmware.command(args.command, args.argument)
    if reply is None:
        raise Failure(f'no reply to feature command {args.command}')
    status, payload = reply
    say(f'{READ_ONLY_COMMANDS[args.command]}: status {status:#04x} '
        f'({FW_STATUS_NAMES.get(status, "?")}), {len(payload)} bytes: {payload.hex()}')


def cmd_enter(args):
    if find_bootloader():
        say('the keyboard is already in update mode')
        return
    with Firmware() as firmware:
        firmware.board.require_update_support()
        reply = firmware.command(FW_CMD_VERSION)
        if reply is None:
            raise Failure('the firmware did not answer the version query; nothing was changed')
        say('firmware version ' + '.'.join(map(str, reply[1][:3])))
        say(f'asking the firmware to reset into update mode (feature command '
            f'{FW_CMD_ENTER_UPDATE_MODE}, argument 0)')
        firmware.reset_into_update_mode()
    entry = wait_for(find_bootloader, MODE_CHANGE_WAIT_S, 'the keyboard in update mode')
    say(f'update mode: {describe(entry)}')


# ---- update mode -------------------------------------------------------------------------------

class Bootloader:
    """The HID interface of the factory bootloader: the keyboard in update mode."""

    def __init__(self):
        self.entry = require_bootloader()
        self.board = board_of_bootloader(self.entry)
        self.device = open_device(self.entry)
        log(f'opened update mode {describe(self.entry)}')

    def __enter__(self):
        return self

    def __exit__(self, *exc_info):
        self.device.close()

    def _send(self, command, parameter):
        drain(self.device, BOOT_READ_SIZE)
        packet = BOOT_MAGIC + bytes([command]) + struct.pack('<I', parameter)
        log(f'-> feature {packet.hex()}')
        self.device.send_feature_report(NO_REPORT_ID + packet)

    def request(self, command, parameter=0, timeout_ms=BOOT_REPLY_TIMEOUT_MS):
        """Send one request and return the body of its reply. Any status but 0x88 raises."""
        self._send(command, parameter)
        data = bytes(self.device.read(BOOT_READ_SIZE, timeout_ms))
        log(f'<- input   {data[:24].hex()}')
        if not data:
            raise BootloaderError(f'no reply to command {command} within {timeout_ms} ms')
        if len(data) < 5 or data[:2] != BOOT_MAGIC or data[2] not in (command, BOOT_ECHO_ANY):
            raise BootloaderError(f'unexpected reply to command {command}: {data[:16].hex()}')
        if data[3] != STATUS_OK:
            raise BootloaderError(f'command {command}: status {data[3]:#04x}')
        return data[5:5 + data[4]]

    def ping(self):
        """Command 0, the first request of a session: tried up to five times, 500 ms apart."""
        for attempt in range(PING_ATTEMPTS):
            try:
                return self.request(BOOT_CMD_PING, timeout_ms=PING_TIMEOUT_MS)
            except (BootloaderError, OSError):
                if attempt == PING_ATTEMPTS - 1:
                    raise
                time.sleep(PING_RETRY_S)

    def send_block(self, index, block):
        """Hand one block to the bootloader as an output report; command 3 then programs it."""
        result = self.device.write(NO_REPORT_ID + block)
        log(f'-> output  block {index}, {len(block)} bytes, result {result}')

    def reboot(self, parameter):
        """Command 5: the bootloader resets the keyboard and sends no reply."""
        self._send(BOOT_CMD_REBOOT, parameter)


def show_bootinfo(boot):
    """Print what the read-only commands 0, 1, 9 and 4 report; return the flag byte or None."""
    show('update mode', describe(boot.entry))
    ping = boot.ping()
    flags = ping[1] if len(ping) > 1 else None
    if flags is None:
        show('flags', f'not reported (ping body {ping.hex()})')
    else:
        names = [name for mask, name in FLAG_NAMES.items() if flags & mask]
        show('flags', f'{flags:#04x} ({", ".join(names) or "none"})')
    version = boot.request(BOOT_CMD_VERSION)
    show('bootloader', 'version ' + '.'.join(map(str, version)))
    serial = boot.request(BOOT_CMD_SERIAL, 0).decode('ascii', 'replace')
    second = boot.request(BOOT_CMD_SERIAL, 1).decode('ascii', 'replace')
    show('serial', serial if second == serial else f'{serial} (with parameter 1: {second})')
    crc = int.from_bytes(boot.request(BOOT_CMD_CRC)[:3], 'little')
    show('session crc', f'{crc:#08x} (blocks written since the bootloader started)')
    return flags


def confirm_flash(blocks):
    prompt = f'type ERASE to invalidate the installed firmware and write {blocks} blocks: '
    try:
        answer = input(prompt)
    except EOFError:
        answer = ''
    if answer != 'ERASE':
        raise Failure('aborted, nothing was changed')


def upload(boot, image):
    """Unlock, start the upload and write every block in ascending order."""
    block = boot.board.block_size
    blocks = len(image) // block
    say(f'command {BOOT_CMD_UNLOCK}: unlock the write commands')
    boot.request(BOOT_CMD_UNLOCK, UNLOCK_KEY)
    say(f'command {BOOT_CMD_START_UPLOAD}: start upload of {blocks} blocks')
    # Command 2 erases the page with the completion marker: from here on the bootloader
    # stays in update mode after every restart until command 5 is sent with bit 0.
    boot.request(BOOT_CMD_START_UPLOAD, blocks)
    started = time.monotonic()
    for index in range(blocks):
        boot.send_block(index, image[index * block:(index + 1) * block])
        boot.request(BOOT_CMD_WRITE_BLOCK, index)
        if index % 100 == 0 or index == blocks - 1:
            print(f'  block {index + 1} / {blocks}')
    say(f'all blocks written in {time.monotonic() - started:.1f} s')


def verify(boot, blocks, expected_crc):
    """Compare the last block index and the CRC the bootloader reports with the image.

    The bootloader answers the write command with 0x88 whatever happened in flash; this
    comparison is the only proof that the image arrived.
    """
    last = int.from_bytes(boot.request(BOOT_CMD_LAST_BLOCK)[:4], 'little')
    if last != blocks - 1:
        raise BootloaderError(f'bootloader reports block {last} as the last one written, '
                              f'expected {blocks - 1}')
    crc = int.from_bytes(boot.request(BOOT_CMD_CRC)[:3], 'little')
    if crc != expected_crc:
        raise BootloaderError(f'CRC mismatch: bootloader {crc:#08x}, image {expected_crc:#08x}')
    show('CRC verified', f'{crc:#08x}')


def cmd_bootinfo(args):
    with Bootloader() as boot:
        show_bootinfo(boot)


def cmd_plan(args):
    plan_image(args.image, args.board)


def cmd_flash(args):
    board, image = plan_image(args.image, args.board)
    board.require_update_support()
    blocks = len(image) // board.block_size
    expected_crc = crc24(image)
    with Bootloader() as boot:
        if boot.board is not board:
            raise Failure(f'the image is for the {board.name}, the keyboard in update mode is a '
                          f'{boot.board.name}; nothing was changed')
        show_bootinfo(boot)
        if not args.yes:
            confirm_flash(blocks)
        try:
            upload(boot, image)
            verify(boot, blocks, expected_crc)
        except (BootloaderError, OSError) as error:
            raise Failure(f'{error}. {STAYS_IN_UPDATE_MODE}') from error
        except KeyboardInterrupt:
            raise Failure(f'interrupted. {STAYS_IN_UPDATE_MODE}') from None
        # Only now, with the CRC matched, may the image be marked as complete.
        say(f'command {BOOT_CMD_REBOOT} with bit 0: '
            'write the completion marker and start the firmware')
        boot.reboot(REBOOT_MARK_COMPLETE)
    entry = wait_for(find_firmware, MODE_CHANGE_WAIT_S, 'the firmware')
    say(f'firmware is back: {describe(entry)}')


def cmd_reboot(args):
    with Bootloader() as boot:
        boot.reboot(args.parameter)
    say(f'reboot requested (command {BOOT_CMD_REBOOT}, parameter {args.parameter})')


def keyboard_present():
    return bool(find_firmware() or find_bootloader())


def wait_for_replug(seconds):
    """Wait until the keyboard has left the bus and is back, in whichever mode."""
    deadline = time.monotonic() + seconds
    say(f'waiting up to {seconds} s for the keyboard to be unplugged')
    while keyboard_present():
        if time.monotonic() > deadline:
            raise Failure('the keyboard was never unplugged, nothing was tested')
        time.sleep(REPLUG_POLL_S)
    say('keyboard unplugged; press the top-right key and the Right arrow fully, then plug it in')
    while not keyboard_present():
        if time.monotonic() > deadline:
            raise Failure('the keyboard did not come back within the waiting time')
        time.sleep(REPLUG_POLL_S)


def cmd_combo(args):
    wait_for_replug(args.wait)
    if not find_bootloader():
        entry = wait_for(find_firmware, FIRMWARE_WAIT_S, 'the firmware')
        say(f'result: the firmware started ({describe(entry)}); '
            'the key combination was NOT detected')
        return
    with Bootloader() as boot:
        flags = show_bootinfo(boot)
        detected = flags is not None and flags & FLAG_KEY_COMBINATION
        say(f'result: update mode; key combination {"DETECTED" if detected else "not reported"} '
            'by the bootloader')
        say(f'release the keys; leaving update mode in {args.hold} s '
            f'(command {BOOT_CMD_REBOOT}, parameter {REBOOT_PLAIN}, writes no flash)')
        time.sleep(args.hold)
        boot.reboot(REBOOT_PLAIN)
    entry = wait_for(find_firmware, MODE_CHANGE_WAIT_S,
                     'the firmware (if the keys are still held, release them and replug)')
    say(f'firmware is back: {describe(entry)}')


# ---- command line ------------------------------------------------------------------------------

def ranged_int(limit):
    """argparse type: an integer in any Python notation (10, 0x0A) from 0 to limit."""
    def convert(text):
        value = int(text, 0)
        if not 0 <= value <= limit:
            raise ValueError(text)
        return value
    convert.__name__ = f'number from 0 to {limit:#x}'
    return convert


def add_command(commands, name, handler, summary, description=None):
    parser = commands.add_parser(name, help=summary, description=description or summary)
    parser.set_defaults(handler=handler)
    return parser


def build_parser():
    parser = argparse.ArgumentParser(
        description='Host tool for openwoot: device information, update mode and flashing.',
        epilog='See tools/README.md for a complete session.')
    parser.add_argument('--log', metavar='FILE',
                        help='append every request and reply, with timestamps, to FILE')
    parser.add_argument('--board', metavar='NAME',
                        help='the board an image is built for (a directory name under boards/); '
                             'needed only where the image does not tell, as a flat binary')
    commands = parser.add_subparsers(dest='action', metavar='COMMAND', required=True)

    add_command(commands, 'info', cmd_info,
                'print identification and state of the running firmware',
                'Query the running firmware: firmware version, USB speed, unique ids, device '
                'configuration, profiles. Sends only commands that read.')

    raw = add_command(commands, 'raw', cmd_raw,
                      'send one read-only feature command to the running firmware',
                      'Send one feature command to the running firmware and print the status and '
                      'the payload of the reply. Only commands that neither write to flash nor '
                      'change the state of the keyboard are sent: '
                      + ', '.join(f'{number} ({name})'
                                  for number, name in sorted(READ_ONLY_COMMANDS.items()))
                      + '. The command table is in docs/protocol.md.')
    raw.add_argument('command', metavar='CMD', type=ranged_int(0xFF), help='command number')
    raw.add_argument('argument', metavar='ARG', type=ranged_int(0xFFFFFFFF), nargs='?', default=0,
                     help='32-bit argument (default 0)')

    add_command(commands, 'enter', cmd_enter,
                'ask the running firmware to reset into update mode',
                'Send feature command 2 to the running firmware and wait until the bootloader '
                'answers. The keyboard then stays in update mode, without typing, until "flash" '
                'or "reboot". Does nothing if the keyboard is in update mode already.')

    add_command(commands, 'bootinfo', cmd_bootinfo,
                'update mode: print flags, bootloader version, serial number and session CRC',
                'Query the bootloader with its read-only commands: why it stayed in update mode '
                '(flags), its version, the serial number, and the CRC of the blocks written '
                'since it started. Changes nothing.')

    plan = add_command(commands, 'plan', cmd_plan,
                       'offline: print block count and CRC of an image, run the sanity checks',
                       'Load an image the way "flash" does, find the board it is built for, check '
                       'that it is firmware for that board (size, initial stack pointer, reset '
                       'vector) and print the number of blocks and the CRC-24 the bootloader must '
                       'report. Does not talk to the keyboard.')
    plan.add_argument('image', metavar='IMAGE', help='Intel HEX file (.hex) or flat binary (.bin)')

    flash = add_command(commands, 'flash', cmd_flash,
                        'update mode: write an image, verify it and start it',
                        'Write an image to the keyboard in update mode: unlock, start the upload, '
                        'send every block, compare the last block index and the CRC with the '
                        'image, and only then mark the firmware as complete and start it. If '
                        'anything fails, the keyboard stays in update mode and the command can '
                        'be repeated.')
    flash.add_argument('image', metavar='IMAGE', help='Intel HEX file (.hex) or flat binary (.bin)')
    flash.add_argument('--yes', action='store_true', help='do not ask for the word ERASE')

    reboot = add_command(commands, 'reboot', cmd_reboot,
                         'leave update mode (bootloader command 5)',
                         'Send bootloader command 5: the keyboard restarts. With an upload begun '
                         'and not finished the bootloader comes back in update mode.')
    reboot.add_argument('parameter', type=int, nargs='?', default=REBOOT_PLAIN,
                        choices=(REBOOT_PLAIN, REBOOT_MARK_COMPLETE),
                        help='0: restart only (default). 1: first mark the content of flash as '
                             'complete firmware, without any check; "flash" does this itself '
                             'after the CRC matched')

    combo = add_command(commands, 'combo', cmd_combo,
                        'check the recovery key combination while the keyboard is replugged',
                        'Wait for the keyboard to be unplugged and plugged in again with the '
                        'top-right key and the Right arrow held, report whether the bootloader '
                        'saw the key combination, then leave update mode without writing flash.')
    combo.add_argument('--wait', type=int, default=600, metavar='SECONDS',
                       help='time allowed for the unplug and the replug (default 600)')
    combo.add_argument('--hold', type=int, default=6, metavar='SECONDS',
                       help='time spent in update mode before leaving it (default 6)')
    return parser


def run(args):
    if args.handler is not cmd_plan and (hid is None or not hasattr(hid, 'device')):
        raise Failure('the Python package "hidapi" is missing; '
                      'install it with: pip install -r tools/requirements.txt')
    if args.log:
        open_log(args.log)
    args.handler(args)


def main():
    args = build_parser().parse_args()
    try:
        run(args)
    except Failure as error:
        message = str(error)
    except OSError as error:
        message = f'USB communication failed: {error}'
    except KeyboardInterrupt:
        message = 'interrupted'
    else:
        return
    log('! ' + message)
    sys.exit('openwoot: ' + message)


if __name__ == '__main__':
    main()
