# Settings schema

`settings.proto` describes the settings files the keyboard stores in its littlefs file system
(profile configuration, RGB settings, actuation points, advanced keys, light-bar effects and so on)
and the protobuf messages the firmware exchanges with the host. The encoding is the one stock
firmware 2.14.1 uses, so stored settings and host tools stay compatible. The files and their
messages are listed in [docs/storage.md](../../docs/storage.md).

The schema is the same for every board. A message that only a feature uses (the light-bar
configuration of feature `lightbar`, the host signals of feature `signals`) is part of the schema
on every board; on a board without the feature nothing reads or writes it, and the linker drops
its descriptor.

`settings.options` holds the nanopb options (array and string sizes, integer widths) that give
the generated C structs the layout the firmware relies on. The key matrices of the schema have
fixed sizes: 6 rows of 21 columns (`KeyMatrixU8`, `KeyMatrixU16`, `KeymapLayer`). A board stores
its key maps and colour maps in that shape, whatever the size of its own matrix
(`BOARD_KEYMAP_COLS` in its `board.h`).

## Generated code

The C code generated from the schema is checked in:

- `src/proto/settings.pb.h`
- `src/proto/settings.pb.c`

The build does not run the generator. Regenerate the two files only after changing the schema or
the options file.

## Regenerating

The generator is nanopb 0.4.9.1, the same version as the runtime in `third_party/nanopb`. It needs
the protobuf compiler `protoc` on the `PATH`.

```
pip install nanopb==0.4.9.1
```

From the repository root:

```
nanopb_generator -I src/proto -f src/proto/settings.options -D src/proto src/proto/settings.proto
```

With an unchanged schema this reproduces the checked-in files byte for byte.

## Names

The stored format and the host protocol only depend on the field numbers and types, never on a
name. The messages `Nameless01` to `Nameless11`, the enum `Nameless02Enum` and the fields named
`f<tag>` or `o<tag>` are parts whose meaning the firmware gives no hint of: it stores them or
passes them on without reading them. Their names are chosen by this project.

Renaming a message changes no behaviour, but it does change the image: the compiler places the
message descriptors of `settings.pb.c` in the order of their names, so the build is no longer
byte-identical with the one before the change.
