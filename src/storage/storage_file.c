/*
 * storage: whole-file helpers on the littlefs volume.
 *
 * Files are opened with lfs_file_opencfg and a cache buffer on the caller's
 * stack (LFS_NO_MALLOC). None of these helpers tests g_storage_mounted; the
 * callers do.
 */
#include <pb_decode.h>

#include "storage/storage.h"

/*
 * Replaces the content of a file (create, truncate, write, close). Returns the
 * number of bytes written or the negative littlefs error of open / write; the
 * result of the close (where littlefs commits) is ignored.
 */
int storage_write_file(const char *path, const void *data, uint32_t size)
{
    uint8_t cache[STORAGE_LFS_CACHE_SIZE];
    struct lfs_file_config file_cfg = { .buffer = cache };
    lfs_file_t file;

    int result = lfs_file_opencfg(&g_storage_lfs, &file, path, STORAGE_OPEN_REWRITE, &file_cfg);
    if (result >= 0) {
        result = lfs_file_write(&g_storage_lfs, &file, data, size);
        lfs_file_close(&g_storage_lfs, &file);
    }
    return result;
}

/* Reads up to size bytes from the start of a file. Returns the number of bytes
 * read or a negative littlefs error (LFS_ERR_NOENT = file missing). */
int storage_read_file(const char *path, void *buf, uint32_t size)
{
    uint8_t cache[STORAGE_LFS_CACHE_SIZE];
    struct lfs_file_config file_cfg = { .buffer = cache };
    lfs_file_t file;

    int result = lfs_file_opencfg(&g_storage_lfs, &file, path, STORAGE_OPEN_READ, &file_cfg);
    if (result >= 0) {
        result = lfs_file_read(&g_storage_lfs, &file, buf, size);
        lfs_file_close(&g_storage_lfs, &file);
    }
    return result;
}

/*
 * 1 if the path exists and its size is not 0, else 0. For a directory littlefs
 * leaves info.size unset, so the result is undefined there; all callers pass
 * file paths.
 */
int storage_file_exists(const char *path)
{
    struct lfs_info info;

    if (lfs_stat(&g_storage_lfs, path, &info) < 0) {
        return 0;
    }
    return info.size != 0u ? 1 : 0;
}

/*
 * Reads a whole file into a stack buffer of the file's size and decodes it
 * into msg (pb_decode applies the message defaults first).
 * Returns STORAGE_OK, STORAGE_ERR_DECODE, or the littlefs error as int16.
 */
int storage_read_pb_file(const char *path, const pb_msgdesc_t *msgdesc, void *msg)
{
    uint8_t cache[STORAGE_LFS_CACHE_SIZE];
    struct lfs_file_config file_cfg = { .buffer = cache };
    lfs_file_t file;

    int err = lfs_file_opencfg(&g_storage_lfs, &file, path, STORAGE_OPEN_READ, &file_cfg);
    if (err < 0) {
        return (int16_t)err;
    }

    lfs_size_t file_size = file.ctz.size;
    uint8_t encoded[file_size];
    lfs_ssize_t length = lfs_file_read(&g_storage_lfs, &file, encoded, file_size);
    lfs_file_close(&g_storage_lfs, &file);
    if (length < 0) {
        return (int16_t)length;
    }

    pb_istream_t stream = pb_istream_from_buffer(encoded, (size_t)length);
    return pb_decode(&stream, msgdesc, msg) ? STORAGE_OK : STORAGE_ERR_DECODE;
}
