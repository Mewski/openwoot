/*
 * storage: files of a profile directory.
 *
 * A profile id selects the directory: namespace 0 -> "<index>", namespace 1 ->
 * "lp/<index>". Every setting group is one small file in it, either a nanopb
 * message or a raw array; each write first increments the revision counter
 * file "pst" of the directory.
 */
#include <stdio.h>
#include <string.h>

#include <pb_encode.h>

#include "storage/storage.h"
#include "profile/profile_types.h"

#define STORAGE_PROFILE_DIR_MAX     8u   /* "lp/255" + NUL */
#define STORAGE_PROFILE_PATH_MAX    28u  /* directory + '/' + numbered file name */
#define STORAGE_AK2_ENCODED_MAX     1040u

/* Directory name of a profile; empty for an unknown namespace. */
static void storage_format_profile_dir(char *out, uint16_t profile_id)
{
    switch (PROFILE_ID_NS(profile_id)) {
    case PROFILE_NS_BUILTIN:
        sprintf(out, "%d", PROFILE_ID_INDEX(profile_id));
        break;
    case PROFILE_NS_LP:
        sprintf(out, "lp/%d", PROFILE_ID_INDEX(profile_id));
        break;
    default:
        out[0] = '\0';
        break;
    }
}

/* Path of a numbered file ("r<k>", "ml<k>", "al<k>") of a profile directory. */
static void storage_format_numbered_path(char *path, uint16_t profile_id, const char *name_format, int number)
{
    char dir[STORAGE_PROFILE_DIR_MAX];
    char name[STORAGE_PROFILE_FILE_NAME_MAX];

    sprintf(name, name_format, number);
    storage_format_profile_dir(dir, profile_id);
    sprintf(path, "%s/%s", dir, name);
}

/*
 * Counts the consecutive existing profile directories of a namespace
 * ("0".."3" or "lp/0".."lp/9"), stopping at the first missing one.
 */
int storage_count_profile_dirs(uint32_t ns, uint8_t *count)
{
    uint32_t max_dirs;

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }

    *count = 0u;
    if (ns == PROFILE_NS_BUILTIN) {
        max_dirs = PROFILE_BUILTIN_MAX;
    } else if (ns == PROFILE_NS_LP) {
        max_dirs = PROFILE_LP_MAX;
    } else {
        return STORAGE_OK;
    }

    for (uint32_t index = 0u; index != max_dirs; index++) {
        char dir_path[STORAGE_PROFILE_DIR_MAX];
        lfs_dir_t dir;

        storage_format_profile_dir(dir_path, PROFILE_ID_RAW(index, ns));
        int err = lfs_dir_open(&g_storage_lfs, &dir, dir_path);
        /* Closed whatever the result: unlinking a handle that is not in the
         * open list does nothing. */
        lfs_dir_close(&g_storage_lfs, &dir);
        if (err == LFS_ERR_NOENT) {
            return STORAGE_OK;
        }
        if (err < 0) {
            return (int16_t)err;
        }
        (*count)++;
    }
    return STORAGE_OK;
}

/*
 * Removes every file of a profile directory, then the directory. Built-in
 * profile 0 is never deleted. A missing directory counts as success.
 */
int storage_delete_profile_dir(uint16_t profile_id)
{
    char dir_path[16];
    char path[STORAGE_PROFILE_PATH_MAX];
    lfs_dir_t dir;
    struct lfs_info info;
    int result;

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_BUILTIN && PROFILE_ID_INDEX(profile_id) == 0u) {
        return STORAGE_ERR_PROTECTED;
    }

    storage_format_profile_dir(dir_path, profile_id);
    result = lfs_dir_open(&g_storage_lfs, &dir, dir_path);
    if (result == LFS_ERR_NOENT) {
        return STORAGE_OK;
    }
    if (result < 0) {
        return (int16_t)result;
    }

    /* A read error ends the iteration like the end of the directory. */
    while (lfs_dir_read(&g_storage_lfs, &dir, &info) > 0) {
        if (info.type == LFS_TYPE_DIR) {
            continue; /* ".", ".." */
        }
        sprintf(path, "%.7s/%.17s", dir_path, info.name);
        result = lfs_remove(&g_storage_lfs, path);
        if (result < 0) {
            /* KNOWN ISSUE: returns without lfs_dir_close, so the directory handle on this
             * stack frame stays linked in g_storage_lfs.mlist. Matches stock firmware 2.14.1. */
            return (int16_t)result;
        }
    }
    lfs_dir_close(&g_storage_lfs, &dir);

    result = lfs_remove(&g_storage_lfs, dir_path);
    if (result < 0) {
        return (int16_t)result;
    }
    return STORAGE_OK;
}

/*
 * Path of the file `name` of a profile: "<index>/<name>" or "lp/<index>/<name>",
 * empty for an unknown namespace. out needs strlen(name) + 8 bytes.
 * The name goes through a 16-byte scratch buffer as a format string, so it
 * must be shorter than 16 characters and free of '%'.
 */
int storage_format_profile_path(char *out, uint16_t profile_id, const char *name)
{
    char name_copy[STORAGE_PROFILE_FILE_NAME_MAX];

    int result = sprintf(name_copy, name);
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_BUILTIN) {
        return sprintf(out, "%d/%s", PROFILE_ID_INDEX(profile_id), name_copy);
    }
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_LP) {
        return sprintf(out, "lp/%d/%s", PROFILE_ID_INDEX(profile_id), name_copy);
    }
    out[0] = '\0';
    return result;
}

/* Reads "<dir>/r<layer>": key colours of one layer, RGB565 by [row][matrix column]. */
int storage_read_layer_colors(uint16_t profile_id, uint32_t layer, void *out)
{
    char path[STORAGE_PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    storage_format_numbered_path(path, profile_id, "r%d", (int)layer);
    int result = storage_read_file(path, out, STORAGE_LAYER_COLORS_SIZE);
    return result < 0 ? (int16_t)result : STORAGE_OK;
}

/* Reads "<dir>/ml<layer>": key map of one layer, 6 x 21 key codes (126 bytes). */
int storage_read_keymap_layer(uint16_t profile_id, void *out, uint32_t layer)
{
    char path[STORAGE_PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    storage_format_numbered_path(path, profile_id, "ml%d", (int)layer);
    int result = storage_read_file(path, out, STORAGE_KEYMAP_LAYER_SIZE);
    return result < 0 ? (int16_t)result : STORAGE_OK;
}

/* Counts the consecutive non-empty files "<dir>/al0".."al15". */
int storage_count_al_files(uint16_t profile_id, uint8_t *count)
{
    char path[STORAGE_PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }

    *count = 0u;
    for (int slot = 0; slot != (int)STORAGE_AL_FILE_MAX; slot++) {
        storage_format_numbered_path(path, profile_id, "al%d", slot);
        if (storage_file_exists(path) == 0) {
            break;
        }
        (*count)++;
    }
    return STORAGE_OK;
}

/*
 * Increments the 32-bit counter file "<dir>/pst" (created with 0 if missing).
 * Skipped while the first-boot defaults are written. Errors are silent.
 */
void storage_bump_profile_revision(uint16_t profile_id)
{
    uint32_t revision;
    char path[12];
    uint8_t cache[STORAGE_LFS_CACHE_SIZE];
    struct lfs_file_config file_cfg = { .buffer = cache };
    lfs_file_t file;

    if (g_storage_revision_bump_disabled != 0u) {
        return;
    }

    storage_format_profile_path(path, profile_id, "pst");
    revision = 0u;
    if (lfs_file_opencfg(&g_storage_lfs, &file, path, STORAGE_OPEN_COUNTER, &file_cfg) < 0) {
        return;
    }
    /* KNOWN ISSUE: the three error exits below leave the file open (the handle on this
     * stack frame stays linked in g_storage_lfs.mlist). Matches stock firmware 2.14.1. */
    if (lfs_file_read(&g_storage_lfs, &file, &revision, sizeof(revision)) < 0) {
        return;
    }
    revision++;
    if (lfs_file_seek(&g_storage_lfs, &file, 0, LFS_SEEK_SET) < 0) {
        return;
    }
    if (lfs_file_write(&g_storage_lfs, &file, &revision, sizeof(revision)) < 0) {
        return;
    }
    lfs_file_close(&g_storage_lfs, &file);
}

/*
 * Writes "<dir>/ml<layer>" (126 bytes). Returns the storage_write_file result;
 * when storage is not mounted it returns the profile id. Matches stock
 * firmware 2.14.1.
 */
int storage_write_keymap_layer(uint16_t profile_id, const void *keymap, uint32_t layer)
{
    char path[STORAGE_PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return profile_id;
    }
    storage_format_numbered_path(path, profile_id, "ml%d", (int)layer);
    storage_bump_profile_revision(profile_id);
    return storage_write_file(path, keymap, STORAGE_KEYMAP_LAYER_SIZE);
}

/* Writes "<dir>/r<layer>" (STORAGE_LAYER_COLORS_SIZE bytes). Return value as storage_write_keymap_layer. */
int storage_write_layer_colors(uint16_t profile_id, uint32_t layer, const void *colors)
{
    char path[STORAGE_PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return profile_id;
    }
    storage_format_numbered_path(path, profile_id, "r%d", (int)layer);
    storage_bump_profile_revision(profile_id);
    return storage_write_file(path, colors, STORAGE_LAYER_COLORS_SIZE);
}

/* Writes "<dir>/clr" (252 bytes). Return value as storage_write_keymap_layer. */
int storage_write_base_colors(uint16_t profile_id, const void *colors)
{
    char path[12];

    if (g_storage_mounted == 0u) {
        return profile_id;
    }
    storage_format_profile_path(path, profile_id, "clr");
    storage_bump_profile_revision(profile_id);
    return storage_write_file(path, colors, STORAGE_BASE_COLORS_SIZE);
}

/*
 * Encodes msg and writes it as "<dir>/<name>". max_size is the size of the
 * encode buffer (stack). The encode result is not checked: a failed encode
 * stores the bytes produced so far. Does not test g_storage_mounted.
 */
int storage_write_profile_pb_file(uint16_t profile_id, const char *name, const pb_msgdesc_t *msgdesc,
                                  uint32_t max_size, const void *msg)
{
    char path[strlen(name) + 8u];

    storage_format_profile_path(path, profile_id, name);
    storage_bump_profile_revision(profile_id);

    uint8_t encoded[max_size];
    pb_ostream_t stream = pb_ostream_from_buffer(encoded, max_size);
    pb_encode(&stream, msgdesc, msg);
    return storage_write_file(path, encoded, (uint16_t)stream.bytes_written);
}

/*
 * Writes the advanced keys as "<dir>/ak2" and removes the legacy file "dks".
 * The revision is bumped twice (here and in the generic writer).
 * Return value as storage_write_keymap_layer.
 */
int storage_save_advanced_keys(uint16_t profile_id, const void *msg)
{
    char legacy_path[12];

    if (g_storage_mounted == 0u) {
        return profile_id;
    }
    storage_format_profile_path(legacy_path, profile_id, "dks");
    lfs_remove(&g_storage_lfs, legacy_path);
    storage_bump_profile_revision(profile_id);
    return storage_write_profile_pb_file(profile_id, "ak2", &AdvancedKeys_msg, STORAGE_AK2_ENCODED_MAX, msg);
}
