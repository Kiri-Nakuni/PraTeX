#!/usr/bin/env python3
"""Compare two standard DVI files while ignoring encoding-only differences.

The comparison keeps every page counter, glyph, rule, explicit movement,
stack operation, special, and postamble bound.  DVI integer widths, file
pointers, padding, font numbers, and redundant font definitions are not part
of the canonical stream.  A glyph instead carries the complete DVI font
identity selected for it.
"""

from __future__ import annotations

import hashlib
import itertools
import pathlib
import sys
from dataclasses import dataclass
from typing import Iterator


class DviError(Exception):
    pass


@dataclass(frozen=True)
class FontIdentity:
    checksum: int
    scale: int
    design_size: int
    area_and_name: bytes


class Reader:
    def __init__(self, data: bytes, path: pathlib.Path) -> None:
        self.data = data
        self.path = path
        self.pos = 0

    def byte(self) -> int:
        if self.pos >= len(self.data):
            raise DviError(f"{self.path}: truncated at byte {self.pos}")
        value = self.data[self.pos]
        self.pos += 1
        return value

    def bytes(self, length: int) -> bytes:
        end = self.pos + length
        if length < 0 or end > len(self.data):
            raise DviError(
                f"{self.path}: record at byte {self.pos} needs {length} bytes"
            )
        value = self.data[self.pos:end]
        self.pos = end
        return value

    def unsigned(self, length: int) -> int:
        if length not in (1, 2, 3, 4):
            raise DviError(f"{self.path}: unsupported integer width {length}")
        return int.from_bytes(self.bytes(length), "big", signed=False)

    def signed(self, length: int) -> int:
        if length not in (1, 2, 3, 4):
            raise DviError(f"{self.path}: unsupported integer width {length}")
        return int.from_bytes(self.bytes(length), "big", signed=True)


def records(path: pathlib.Path) -> Iterator[tuple]:
    reader = Reader(path.read_bytes(), path)
    saw_post_post = False
    while reader.pos < len(reader.data):
        offset = reader.pos
        opcode = reader.byte()
        if 0 <= opcode <= 127:
            yield ("set_char", opcode, offset)
        elif 128 <= opcode <= 131:
            yield ("set_char", reader.unsigned(opcode - 127), offset)
        elif opcode == 132:
            yield ("set_rule", reader.signed(4), reader.signed(4), offset)
        elif 133 <= opcode <= 136:
            yield ("put_char", reader.unsigned(opcode - 132), offset)
        elif opcode == 137:
            yield ("put_rule", reader.signed(4), reader.signed(4), offset)
        elif opcode == 138:
            yield ("nop", offset)
        elif opcode == 139:
            counts = tuple(reader.signed(4) for _ in range(10))
            previous = reader.signed(4)
            yield ("bop", counts, previous, offset)
        elif opcode == 140:
            yield ("eop", offset)
        elif opcode == 141:
            yield ("push", offset)
        elif opcode == 142:
            yield ("pop", offset)
        elif 143 <= opcode <= 146:
            yield ("right", reader.signed(opcode - 142), offset)
        elif opcode == 147:
            yield ("w0", offset)
        elif 148 <= opcode <= 151:
            yield ("w", reader.signed(opcode - 147), offset)
        elif opcode == 152:
            yield ("x0", offset)
        elif 153 <= opcode <= 156:
            yield ("x", reader.signed(opcode - 152), offset)
        elif 157 <= opcode <= 160:
            yield ("down", reader.signed(opcode - 156), offset)
        elif opcode == 161:
            yield ("y0", offset)
        elif 162 <= opcode <= 165:
            yield ("y", reader.signed(opcode - 161), offset)
        elif opcode == 166:
            yield ("z0", offset)
        elif 167 <= opcode <= 170:
            yield ("z", reader.signed(opcode - 166), offset)
        elif 171 <= opcode <= 234:
            yield ("font", opcode - 171, offset)
        elif 235 <= opcode <= 238:
            yield ("font", reader.unsigned(opcode - 234), offset)
        elif 239 <= opcode <= 242:
            length = reader.unsigned(opcode - 238)
            yield ("special", reader.bytes(length), offset)
        elif 243 <= opcode <= 246:
            number = reader.unsigned(opcode - 242)
            checksum = reader.unsigned(4)
            scale = reader.unsigned(4)
            design_size = reader.unsigned(4)
            area_length = reader.byte()
            name_length = reader.byte()
            identity = FontIdentity(
                checksum,
                scale,
                design_size,
                reader.bytes(area_length + name_length),
            )
            yield ("font_def", number, identity, offset)
        elif opcode == 247:
            dvi_id = reader.byte()
            numerator = reader.unsigned(4)
            denominator = reader.unsigned(4)
            magnification = reader.unsigned(4)
            comment = reader.bytes(reader.byte())
            yield (
                "pre",
                dvi_id,
                numerator,
                denominator,
                magnification,
                comment,
                offset,
            )
        elif opcode == 248:
            previous_bop = reader.signed(4)
            numerator = reader.unsigned(4)
            denominator = reader.unsigned(4)
            magnification = reader.unsigned(4)
            max_height = reader.unsigned(4)
            max_width = reader.unsigned(4)
            max_stack = reader.unsigned(2)
            pages = reader.unsigned(2)
            yield (
                "post",
                previous_bop,
                numerator,
                denominator,
                magnification,
                max_height,
                max_width,
                max_stack,
                pages,
                offset,
            )
        elif opcode == 249:
            post_pointer = reader.signed(4)
            dvi_id = reader.byte()
            padding = reader.bytes(len(reader.data) - reader.pos)
            if len(padding) < 4 or any(value != 223 for value in padding):
                raise DviError(f"{path}: invalid post_post padding at byte {offset}")
            yield ("post_post", post_pointer, dvi_id, offset)
            saw_post_post = True
        else:
            raise DviError(f"{path}: undefined DVI opcode {opcode} at byte {offset}")
    if not saw_post_post:
        raise DviError(f"{path}: missing post_post")


def font_table(path: pathlib.Path) -> dict[int, FontIdentity]:
    fonts: dict[int, FontIdentity] = {}
    for record in records(path):
        if record[0] != "font_def":
            continue
        _, number, identity, offset = record
        previous = fonts.setdefault(number, identity)
        if previous != identity:
            raise DviError(
                f"{path}: font {number} is redefined differently at byte {offset}"
            )
    return fonts


def canonical(path: pathlib.Path) -> Iterator[tuple]:
    fonts = font_table(path)
    current_font: FontIdentity | None = None
    registers = [0, 0, 0, 0]  # w, x, y, z
    stack: list[tuple[int, int, int, int]] = []
    in_page = False
    saw_pre = False
    saw_post = False
    pages = 0

    for record in records(path):
        kind = record[0]
        offset = record[-1]
        if kind == "pre":
            if saw_pre or offset != 0:
                raise DviError(f"{path}: misplaced or duplicate preamble at byte {offset}")
            saw_pre = True
            yield record[:-1]
        elif kind == "bop":
            if in_page or stack or saw_post:
                raise DviError(f"{path}: invalid bop state at byte {offset}")
            in_page = True
            pages += 1
            current_font = None
            registers = [0, 0, 0, 0]
            yield ("bop", record[1])  # Previous-page file pointers are encoding only.
        elif kind == "eop":
            if not in_page or stack:
                raise DviError(f"{path}: invalid eop state at byte {offset}")
            in_page = False
            yield ("eop",)
        elif kind == "push":
            if not in_page:
                raise DviError(f"{path}: push outside a page at byte {offset}")
            stack.append(tuple(registers))
            yield ("push",)
        elif kind == "pop":
            if not in_page or not stack:
                raise DviError(f"{path}: stack underflow at byte {offset}")
            registers[:] = stack.pop()
            yield ("pop",)
        elif kind == "font":
            if not in_page:
                raise DviError(f"{path}: font selection outside a page at byte {offset}")
            try:
                current_font = fonts[record[1]]
            except KeyError as error:
                raise DviError(
                    f"{path}: undefined font {record[1]} selected at byte {offset}"
                ) from error
        elif kind in ("set_char", "put_char"):
            if not in_page or current_font is None:
                raise DviError(f"{path}: glyph without a font at byte {offset}")
            yield (kind, current_font, record[1])
        elif kind in ("set_rule", "put_rule"):
            if not in_page:
                raise DviError(f"{path}: rule outside a page at byte {offset}")
            yield record[:-1]
        elif kind == "right":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("right", record[1])
        elif kind == "w0":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("right", registers[0])
        elif kind == "w":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            registers[0] = record[1]
            yield ("right", registers[0])
        elif kind == "x0":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("right", registers[1])
        elif kind == "x":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            registers[1] = record[1]
            yield ("right", registers[1])
        elif kind == "down":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("down", record[1])
        elif kind == "y0":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("down", registers[2])
        elif kind == "y":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            registers[2] = record[1]
            yield ("down", registers[2])
        elif kind == "z0":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            yield ("down", registers[3])
        elif kind == "z":
            if not in_page:
                raise DviError(f"{path}: movement outside a page at byte {offset}")
            registers[3] = record[1]
            yield ("down", registers[3])
        elif kind == "special":
            if not in_page:
                raise DviError(f"{path}: special outside a page at byte {offset}")
            yield ("special", record[1])
        elif kind in ("nop", "font_def"):
            continue
        elif kind == "post":
            if in_page or stack or saw_post:
                raise DviError(f"{path}: postamble inside a page at byte {offset}")
            if record[8] != pages:
                raise DviError(
                    f"{path}: postamble declares {record[8]} pages after {pages} bop records"
                )
            saw_post = True
            # File pointers are encoding only; all declared bounds remain semantic.
            yield ("post",) + record[2:-1]
        elif kind == "post_post":
            yield ("post_post", record[2])
        else:
            raise DviError(f"{path}: unhandled record {kind} at byte {offset}")

    if not saw_pre or not saw_post or in_page or stack:
        raise DviError(f"{path}: incomplete DVI structure")


def compare(left: pathlib.Path, right: pathlib.Path) -> int:
    left_hash = hashlib.sha256()
    right_hash = hashlib.sha256()
    count = 0
    sentinel = object()
    for index, (left_event, right_event) in enumerate(
        itertools.zip_longest(canonical(left), canonical(right), fillvalue=sentinel),
        start=1,
    ):
        if left_event is not sentinel:
            left_hash.update(repr(left_event).encode("utf-8"))
            left_hash.update(b"\n")
        if right_event is not sentinel:
            right_hash.update(repr(right_event).encode("utf-8"))
            right_hash.update(b"\n")
        if left_event != right_event:
            print(f"DVI semantic difference at canonical record {index}", file=sys.stderr)
            print(f"left:  {left_event!r}", file=sys.stderr)
            print(f"right: {right_event!r}", file=sys.stderr)
            return 1
        count = index
    digest = left_hash.hexdigest()
    if digest != right_hash.hexdigest():
        raise DviError("equal event streams produced different canonical hashes")
    print(f"DVI semantics match: records={count} sha256={digest}")
    return 0


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} LEFT.dvi RIGHT.dvi", file=sys.stderr)
        return 2
    left = pathlib.Path(sys.argv[1])
    right = pathlib.Path(sys.argv[2])
    try:
        return compare(left, right)
    except (DviError, OSError) as error:
        print(error, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
