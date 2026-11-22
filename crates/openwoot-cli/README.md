# openwoot-cli

Host tool `openwoot` for openwoot keyboards.

```sh
openwoot flash --dry-run build/wooting-80he/openwoot-wooting-80he.hex
```

| Command | Does |
| --- | --- |
| `openwoot flash IMAGE --dry-run` | reads an Intel HEX file or flat binary, finds the image descriptor, checks the image against it and prints the board, the USB ids and the blocks and CRC-24 an update would send; touches no device |
| `openwoot flash IMAGE` | not implemented yet: refuses after the checks |
| `openwoot version` | not implemented yet |

## Linux

`udev/70-openwoot.rules` lets the logged-in user open the keyboard's HID devices:

```sh
sudo cp udev/70-openwoot.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

then unplug the keyboard and plug it in again.
